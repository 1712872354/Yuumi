//! 游戏阶段转换 → 上传触发。

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use super::queue::UploadQueue;
use super::url::should_trigger_upload_on_phase;

/// 游戏状态转换上下文。由 auto_match agent 维护。
pub struct UploadTrigger {
    prev_phase: String,
    queue: Arc<UploadQueue>,
    puuid: Arc<Mutex<Option<String>>>,
    last_game_id: Arc<Mutex<u64>>,
    current_game_id: Arc<Mutex<Option<u64>>>,
    /// 标记当前对局的上传是否已完成，避免退回 Lobby/None 时重复查询 LCU
    upload_completed: Arc<Mutex<bool>>,
}

impl UploadTrigger {
    pub fn new(queue: Arc<UploadQueue>) -> Self {
        Self {
            prev_phase: "None".to_string(),
            queue,
            puuid: Arc::new(Mutex::new(None)),
            last_game_id: Arc::new(Mutex::new(0)),
            current_game_id: Arc::new(Mutex::new(None)),
            upload_completed: Arc::new(Mutex::new(false)),
        }
    }

    /// 检测游戏状态转换，符合条件时触发上传。
    ///
    /// 触发条件：当前阶段为 EndOfGame/Lobby/None，
    /// 且前一阶段为 InProgress/GameStart/PreEndOfGame/Reconnect。
    pub async fn on_phase_change(&mut self, phase: &str, app_handle: &AppHandle) {
        let prev = self.prev_phase.clone();
        self.prev_phase = phase.to_string();

        if matches!(phase, "ChampSelect" | "ReadyCheck") {
            *self.current_game_id.lock().await = None;
        }

        if matches!(phase, "GameStart" | "InProgress") {
            *self.upload_completed.lock().await = false;
            match fetch_gameflow_game_id(app_handle).await {
                Ok(Some(id)) => {
                    log::info!("记录当前对局 ID: {} (phase={})", id, phase);
                    *self.current_game_id.lock().await = Some(id);
                }
                Ok(None) => log::debug!("当前 gameflow session 暂无 gameId (phase={})", phase),
                Err(e) => log::debug!("读取当前对局 ID 失败 (phase={}): {}", phase, e),
            }
            return;
        }

        // 只在游戏结束后触发
        if !should_trigger_upload_on_phase(phase, &prev) {
            return;
        }

        log::info!("检测到游戏结束转换: {} → {}，准备上传...", prev, phase);

        // 如果该对局已完成上传或正在上传中，跳过重复处理
        {
            let mut completed = self.upload_completed.lock().await;
            if *completed {
                log::debug!("该对局的上传已完成或处理中，跳过重复处理 (phase={})", phase);
                return;
            }
            // 立即标记为处理中，防止 Phase 抖动叠加多个异步任务
            *completed = true;
        }

        let queue = self.queue.clone();
        let puuid_cache = self.puuid.clone();
        let last_game_id = self.last_game_id.clone();
        let current_game_id = self.current_game_id.clone();
        let upload_completed = self.upload_completed.clone();
        let app_handle = app_handle.clone();
        let phase_str = phase.to_string();

        crate::spawn_log_panic(async move {
            // 延迟 2 秒等待 LCU 数据写入
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            // 获取 puuid（首次需要查询，后续缓存）
            let puuid = match fetch_or_cache_puuid(&puuid_cache, &app_handle).await {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("无法获取 puuid，跳过上传: {}", e);
                    *upload_completed.lock().await = false;
                    return;
                }
            };

            // 优先使用游戏进行中记录的当前对局 ID；LCU 战绩列表在结算后可能短暂滞后。
            let game_id = match *current_game_id.lock().await {
                Some(id) => id,
                None => match fetch_latest_game_id(&app_handle, &puuid).await {
                    Ok(id) => id,
                    Err(e) => {
                        log::warn!("无法获取最近对局 ID，跳过上传: {}", e);
                        *upload_completed.lock().await = false;
                        return;
                    }
                },
            };
            // 去重：与上次上传对比
            let mut last_id = last_game_id.lock().await;
            if game_id == *last_id {
                log::debug!("对局 {} 已上传过，跳过 (phase={})", game_id, phase_str);
                *current_game_id.lock().await = None;
                return;
            }
            *last_id = game_id;
            drop(last_id);

            // 推入上传队列
            queue.enqueue(game_id).await;
            // 下一场对局结束时，顺带触发重试一次以往暂存未完成的战绩
            queue.trigger_pending_retry().await;
            *current_game_id.lock().await = None;
        });
    }
}

async fn fetch_or_cache_puuid(
    puuid_cache: &Arc<Mutex<Option<String>>>,
    app_handle: &AppHandle,
) -> Result<String, String> {
    // 检查缓存
    {
        let cached = puuid_cache.lock().await;
        if let Some(ref p) = *cached {
            return Ok(p.clone());
        }
    }

    // 从 LCU 获取（锁内只提取连接参数，立即释放读锁）
    let state = app_handle.state::<crate::AppState>();
    let (port, token, http_client) = {
        let lock = state.lcu.client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!(
        "https://127.0.0.1:{}/lol-summoner/v1/current-summoner",
        port
    );
    let auth = crate::build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("请求当前召唤师信息失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取当前召唤师信息: HTTP {}", resp.status()));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析当前召唤师信息失败: {}", e))?;

    let puuid = data
        .get("puuid")
        .and_then(|v| v.as_str())
        .ok_or("当前召唤师信息中缺少 puuid 字段")?
        .to_string();

    // 缓存
    let mut cached = puuid_cache.lock().await;
    *cached = Some(puuid.clone());

    Ok(puuid)
}

pub(crate) fn extract_gameflow_game_id(data: &Value) -> Option<u64> {
    data.get("gameData")
        .and_then(|game_data| game_data.get("gameId"))
        .and_then(|id| id.as_u64())
        .filter(|id| *id > 0)
}

async fn fetch_gameflow_game_id(app_handle: &AppHandle) -> Result<Option<u64>, String> {
    let state = app_handle.state::<crate::AppState>();
    // 锁内只提取连接参数，立即释放读锁
    let (port, token, http_client) = {
        let lock = state.lcu.client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!("https://127.0.0.1:{}/lol-gameflow/v1/session", port);
    let auth = crate::build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("请求 gameflow session 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取 gameflow session: HTTP {}", resp.status()));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 gameflow session 失败: {}", e))?;

    Ok(extract_gameflow_game_id(&data))
}

/// 获取最新一局的 gameId
async fn fetch_latest_game_id(app_handle: &AppHandle, puuid: &str) -> Result<u64, String> {
    let state = app_handle.state::<crate::AppState>();
    // 锁内只提取连接参数，立即释放读锁
    let (port, token, http_client) = {
        let lock = state.lcu.client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=1",
        port, puuid
    );
    let auth = crate::build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("请求最近对局列表失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取最近对局列表: HTTP {}", resp.status()));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析最近对局列表失败: {}", e))?;

    let games_arr = data
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|g| g.as_array());

    match games_arr.and_then(|arr| arr.first()) {
        Some(first) => first
            .get("gameId")
            .and_then(|id| id.as_u64())
            .ok_or_else(|| "最近对局条目缺少 gameId 字段".to_string()),
        None => Err("最近对局列表为空".to_string()),
    }
}
