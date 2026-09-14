//! 单局/批量上传 HTTP 服务层。

use std::sync::{Arc, OnceLock};

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::payload::{build_upload_payload, GameDetail, UploadPayload};
use super::store::{
    add_or_update_pending_upload, load_pending_uploads, remove_pending_upload, UploadTask,
};

/// 创建用于外部 API 的 reqwest Client（正常 SSL 验证，30 秒超时）。
/// 与 LCU Client（danger_accept_invalid_certs=true）分开，避免影响外部 HTTPS 请求。
/// 复用同一个 Client（OnceLock），避免每次上传重复 TLS 握手与连接池重建。
fn external_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// 执行单场对局上传任务（支持自动离线降级与落盘存盘）
pub async fn upload_single_task(
    app_handle: &AppHandle,
    task: UploadTask,
    upload_url: &str,
) -> Result<String, String> {
    let payload = match task {
        UploadTask::Pending(item) => item.payload,
        UploadTask::GameId(game_id) => {
            match fetch_payload_from_lcu(app_handle, game_id).await {
                Ok(p) => p,
                Err(err_msg) => {
                    // 无法从 LCU 拉取详情时，查找是否有之前已被落盘的 pending 记录
                    let existing = load_pending_uploads(app_handle)
                        .into_iter()
                        .find(|i| i.game_id == game_id);
                    if let Some(item) = existing {
                        log::info!(
                            "LCU 暂无法获取详情，使用本地暂存的对局 {} 数据尝试上传",
                            game_id
                        );
                        item.payload
                    } else {
                        return Err(err_msg);
                    }
                }
            }
        }
    };

    let game_id = payload.match_info.match_id;

    match post_payload_to_api(&payload, upload_url).await {
        Ok(status) => {
            remove_pending_upload(app_handle, game_id);
            Ok(status)
        }
        Err(err_msg) => {
            log::warn!(
                "对局 {} API 请求失败，保存暂存数据至 SQLite 数据库: {}",
                game_id,
                err_msg
            );
            add_or_update_pending_upload(app_handle, payload, err_msg.clone());
            Err(err_msg)
        }
    }
}

/// 从 LCU 拉取对局详情并构建 Smart Split Payload
async fn fetch_payload_from_lcu(
    app_handle: &AppHandle,
    game_id: u64,
) -> Result<UploadPayload, String> {
    let (lcu_client, auth, base) = {
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu.client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (
            lcu.http_client.clone(),
            crate::build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
        )
    };

    let current_summoner_info = get_current_summoner_info(&lcu_client, &auth, &base).await;
    let current_puuid = current_summoner_info
        .as_ref()
        .map(|(_, puuid)| puuid.as_str());

    let detail_url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
    let resp = lcu_client
        .get(&detail_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| format!("获取对局详情失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取对局详情: HTTP {}", resp.status()));
    }

    let game_detail: GameDetail = resp
        .json()
        .await
        .map_err(|e| format!("解析对局详情失败: {}", e))?;

    let champion_names = {
        let state = app_handle.state::<crate::AppState>();
        let gd = state.lcu.game_data.read().await;
        gd.champions.clone()
    };
    let payload = build_upload_payload(&game_detail, current_puuid, &champion_names);

    log::info!(
        "对局数据构建完成: matchId={}, 内层{}人, 外层{}人",
        payload.match_info.match_id,
        payload.match_info.participants.len(),
        payload.participants.len()
    );

    Ok(payload)
}

/// 发送 UploadPayload 至外部 API（内部最多重试 5 次）
async fn post_payload_to_api(payload: &UploadPayload, upload_url: &str) -> Result<String, String> {
    let game_id = payload.match_info.match_id;
    let ext_client = external_http_client();
    let mut last_err = String::new();

    for retry in 0..=5 {
        match ext_client
            .post(upload_url)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if json
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            let data = json.get("data");
                            let is_new = data
                                .and_then(|d| d.get("isNewMatch"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            if is_new {
                                log::info!("对局 {} 上传成功 (new)", game_id);
                                return Ok("new".to_string());
                            } else {
                                log::info!("对局 {} 已存在 (exists)", game_id);
                                return Ok("exists".to_string());
                            }
                        }
                        let msg = json
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("未知错误");
                        last_err = format!("服务器处理失败: {}", msg);
                    } else {
                        last_err = "解析响应失败".to_string();
                    }
                } else if resp.status().is_client_error() {
                    // 4xx 客户端错误（参数/鉴权等）重试无意义，直接放弃
                    log::warn!(
                        "对局 {} 上传被拒绝: HTTP {}，不重试",
                        game_id,
                        resp.status()
                    );
                    return Err(format!(
                        "对局 {} 上传失败: HTTP {} (客户端错误不重试)",
                        game_id,
                        resp.status()
                    ));
                } else {
                    last_err = format!("HTTP {}", resp.status());
                }
            }
            Err(e) => {
                last_err = format!("请求失败: {}", e);
            }
        }

        if retry < 5 {
            log::warn!(
                "对局 {} 上传失败 ({}), 重试 {}/5",
                game_id,
                last_err,
                retry + 1
            );
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    Err(format!("对局 {} 上传失败: {}", game_id, last_err))
}

pub(super) async fn get_current_summoner_info(
    http: &reqwest::Client,
    auth: &str,
    base: &str,
) -> Option<(String, String)> {
    let url = format!("{}/lol-summoner/v1/current-summoner", base);
    let resp = match http.get(&url).header("Authorization", auth).send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("获取召唤师信息失败: {}", e);
            return None;
        }
    };
    let data: Value = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("解析召唤师信息失败: {}", e);
            return None;
        }
    };
    let puuid = data.get("puuid")?.as_str()?.to_string();
    let name = data.get("gameName")?.as_str()?.to_string();
    Some((name, puuid))
}

/// 批量上传结果
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUploadResult {
    pub success_count: u32,
    pub failed_count: u32,
    pub error: Option<String>,
}

/// 通过 gameId 列表批量上传对局数据（自动分批，每批 10 场）。
pub async fn batch_upload_by_ids(
    app_handle: &AppHandle,
    game_ids: &[u64],
    batch_url: &str,
) -> BatchUploadResult {
    let (lcu_client, auth, base) = {
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu.client.read().await;
        match lock.as_ref() {
            Some(lcu) => (
                lcu.http_client.clone(),
                crate::build_auth_header(&lcu.token),
                format!("https://127.0.0.1:{}", lcu.port),
            ),
            None => {
                return BatchUploadResult {
                    success_count: 0,
                    failed_count: game_ids.len() as u32,
                    error: Some("LCU 未连接".to_string()),
                };
            }
        }
    };

    let current_summoner_info = get_current_summoner_info(&lcu_client, &auth, &base).await;
    let current_puuid = current_summoner_info
        .as_ref()
        .map(|(_, puuid)| puuid.as_str());

    // 获取英雄名称映射（Arc 共享，避免并发任务各自深拷贝）
    let champion_names = {
        let state = app_handle.state::<crate::AppState>();
        let gd = state.lcu.game_data.read().await;
        Arc::new(gd.champions.clone())
    };

    // 并发获取对局详情并构建 payload（限流并发 10，避免请求风暴）
    use futures_util::StreamExt;
    let payloads: Vec<UploadPayload> = futures_util::stream::iter(game_ids.iter().copied())
        .map(|game_id| {
            let lcu_client = lcu_client.clone();
            let auth = auth.clone();
            let base = base.clone();
            let current_puuid = current_puuid.map(str::to_owned);
            let champion_names = champion_names.clone();
            async move {
                let detail_url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
                match lcu_client
                    .get(&detail_url)
                    .header("Authorization", &auth)
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<GameDetail>().await {
                            Ok(detail) => Some(build_upload_payload(
                                &detail,
                                current_puuid.as_deref(),
                                &champion_names,
                            )),
                            Err(e) => {
                                log::warn!("批量上传: 解析对局 {} 详情失败: {}", game_id, e);
                                None
                            }
                        }
                    }
                    Ok(resp) => {
                        log::warn!(
                            "批量上传: 获取对局 {} 详情失败: HTTP {}",
                            game_id,
                            resp.status()
                        );
                        None
                    }
                    Err(e) => {
                        log::warn!("批量上传: 获取对局 {} 详情请求失败: {}", game_id, e);
                        None
                    }
                }
            }
        })
        .buffer_unordered(10)
        .filter_map(|x| async move { x })
        .collect()
        .await;

    if payloads.is_empty() {
        log::error!("批量上传: 全部 {} 场对局详情获取失败", game_ids.len());
        return BatchUploadResult {
            success_count: 0,
            failed_count: game_ids.len() as u32,
            error: Some(format!("全部 {} 场对局详情获取失败", game_ids.len())),
        };
    }

    let total = payloads.len() as u32;

    // 分批上传（每批 10 场）
    let mut total_success: u32 = 0;
    let mut total_failed: u32 = 0;
    let mut error_msg: Option<String> = None;

    let ext_client = external_http_client();

    for (i, chunk) in payloads.chunks(10).enumerate() {
        log::info!("批量上传第 {} 批，本批 {} 场对局", i + 1, chunk.len());

        match ext_client
            .post(batch_url)
            .header("Content-Type", "application/json")
            .json(chunk)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let status = resp.status();
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if json
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            let data = json.get("data");
                            let success = data
                                .and_then(|d| d.get("successCount").or(d.get("newMatches")))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(chunk.len() as u64)
                                as u32;
                            let failed = data
                                .and_then(|d| d.get("failedCount"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            total_success += success;
                            total_failed += failed;
                            log::info!(
                                "批量上传第 {} 批完成: 成功={}, 失败={}",
                                i + 1,
                                success,
                                failed
                            );
                        } else {
                            let msg = json
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("未知错误");
                            total_failed += chunk.len() as u32;
                            error_msg = Some(format!("服务器处理失败: {}", msg));
                            log::warn!("批量上传第 {} 批服务器返回失败: {}", i + 1, msg);
                        }
                    }
                    Err(e) => {
                        total_failed += chunk.len() as u32;
                        error_msg = Some(format!("解析响应 JSON 失败 (HTTP {}): {}", status, e));
                        log::warn!("批量上传第 {} 批解析响应失败: {}", i + 1, e);
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let preview: String = body.chars().take(200).collect();
                total_failed += chunk.len() as u32;
                error_msg = Some(format!("HTTP {}: {}", status, preview));
                log::warn!("批量上传第 {} 批 HTTP 错误 {}: {}", i + 1, status, preview);
            }
            Err(e) => {
                total_failed += chunk.len() as u32;
                let detail = format!("{}", e);
                log::error!("批量上传连接失败，停止后续批次: {}", detail);
                error_msg = Some(format!("连接失败: {}", detail));
                break;
            }
        }
    }

    log::info!(
        "批量上传完成: 成功={}, 失败={}, 总计={}",
        total_success,
        total_failed,
        total
    );

    BatchUploadResult {
        success_count: total_success,
        failed_count: total_failed,
        error: error_msg,
    }
}
