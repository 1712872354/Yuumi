use serde::Deserialize;
use tauri::State;

use crate::config::WEGAME_MARKER;
use crate::{build_auth_header, AppState};

#[derive(Deserialize)]
pub struct SpectateDirectlyParams {
    pub summoner_name: String,
}

/// CMD 方式观战：通过 SGP 获取观战凭据，直接启动 League of Legends.exe。
/// 与 LCU API 方式（/lol-spectator/v1/spectate/launch）相比，可绕开
/// "Already in gameflow" 错误，无需等待客户端 gameflow 状态切换。
#[tauri::command]
pub async fn spectate_directly(
    params: SpectateDirectlyParams,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let name = params.summoner_name.trim().to_string();
    if name.is_empty() {
        return Err("请输入召唤师名称".to_string());
    }

    // 尽早释放读锁，避免跨多个 HTTP await（含最长 15s 超时的 SGP 请求）持有锁阻塞 monitor 重连写锁
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client, server) = (lcu.port, lcu.token, lcu.http_client, lcu.server);
    let auth = build_auth_header(&token);
    let lcu_base = format!("https://127.0.0.1:{}", port);

    // ── 1. 获取大区标识 ──
    let server = server
        .ok_or_else(|| "无法获取大区信息（--rso_platform_id），请重启客户端后重试".to_string())?;
    let server_lower = server.to_lowercase();

    if !crate::lcu::sgp::is_tencent_server(&server_lower) {
        return Err(format!(
            "CMD 观战仅支持腾讯大区，当前大区 {} 不支持",
            server
        ));
    }

    // ── 2. 通过 LCU 获取召唤师 puuid ──
    let summoner_url = format!("{}/lol-summoner/v1/summoners", lcu_base);
    let summoner_resp = http_client
        .get(&summoner_url)
        .header("Authorization", &auth)
        .query(&[("name", &name)])
        .send()
        .await
        .map_err(|e| format!("获取召唤师信息失败: {}", e))?;

    if !summoner_resp.status().is_success() {
        return Err(format!(
            "未找到召唤师 \"{}\" (HTTP {})",
            name,
            summoner_resp.status().as_u16()
        ));
    }

    let summoner_data: serde_json::Value = summoner_resp
        .json()
        .await
        .map_err(|e| format!("解析召唤师数据失败: {}", e))?;
    let puuid = summoner_data
        .get("puuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "召唤师数据中缺少 puuid".to_string())?
        .to_string();

    // ── 3. 获取 SGP accessToken（30 分钟缓存复用）与共享客户端 ──
    let sgp_token = crate::lcu::sgp::get_sgp_token(port, &auth).await?;

    // ── 4. 构建 SGP base URL 并请求观战凭据 ──
    let sgp_base = crate::lcu::sgp::sgp_base_url(&server_lower);

    let sgp_client = crate::lcu::sgp::get_sgp_client();

    let sgp_url = format!(
        "{}/gsm/v1/ledge/spectator/region/{}/puuid/{}",
        sgp_base, server, puuid
    );

    log::info!("CMD 观战: 请求 SGP 完整 URL = {}", sgp_url);

    let sgp_resp = sgp_client
        .get(&sgp_url)
        .header("Authorization", format!("Bearer {}", sgp_token))
        .send()
        .await
        .map_err(|e| format!("SGP 请求失败: {}", e))?;

    if !sgp_resp.status().is_success() {
        let status = sgp_resp.status();
        let body = sgp_resp.text().await.unwrap_or_default();
        log::warn!("SGP 观战请求失败: HTTP {}, body: {}", status, body);

        let friendly_err = if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
            || body.contains("NOT_IN_GAME")
            || body.contains("not found")
        {
            "该召唤师当前不在游戏中".to_string()
        } else {
            format!("获取观战数据失败 (HTTP {})", status.as_u16())
        };
        return Err(friendly_err);
    }

    let sgp_data: serde_json::Value = sgp_resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP 响应失败: {}", e))?;

    let credentials = sgp_data
        .get("playerCredentials")
        .ok_or_else(|| "该召唤师当前不在游戏中".to_string())?;

    let observer_ip = credentials
        .get("observerServerIp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "观战凭据缺少 observerServerIp".to_string())?;
    let observer_port = credentials
        .get("observerServerPort")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "观战凭据缺少 observerServerPort".to_string())?;
    let encryption_key = credentials
        .get("observerEncryptionKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "观战凭据缺少 observerEncryptionKey".to_string())?;
    let game_id = credentials
        .get("gameId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "观战凭据缺少 gameId".to_string())?;

    // ── 5. 定位 Game 目录并启动 League of Legends.exe ──
    let cfg = app_state.config.read().await;
    // 跳过 WeGame 标记，取第一个真实客户端路径
    let lol_path = cfg
        .general
        .lol_path
        .iter()
        .find(|p| *p != WEGAME_MARKER)
        .cloned()
        .ok_or_else(|| "未配置英雄联盟客户端路径，请在设置中配置".to_string())?;
    drop(cfg);

    // 优先尝试 lol_path/Game（Yuumi 配置的是含 LeagueClient.exe 的根目录）
    // 回退尝试 lol_path/../Game（兼容 lol_path 指向 LeagueClient 子目录的情况）
    let game_dir = {
        let primary = std::path::Path::new(&lol_path).join("Game");
        if primary.join("League of Legends.exe").exists() {
            primary
        } else {
            let fallback = std::path::Path::new(&lol_path)
                .parent()
                .map(|p| p.join("Game"))
                .unwrap_or(primary.clone());
            if fallback.join("League of Legends.exe").exists() {
                fallback
            } else {
                return Err(format!(
                    "未找到游戏可执行文件。\n尝试过:\n  {}\n  {}\n请在设置中确认客户端安装路径",
                    primary.join("League of Legends.exe").display(),
                    fallback.join("League of Legends.exe").display()
                ));
            }
        }
    };
    let game_exe = game_dir.join("League of Legends.exe");

    log::info!(
        "CMD 观战: 启动 {:?} spectator {}:{} {} {} {} (cwd={:?})",
        game_exe,
        observer_ip,
        observer_port,
        encryption_key,
        game_id,
        server,
        game_dir
    );

    std::process::Command::new(&game_exe)
        .args([
            "spectator",
            &format!("{}:{}", observer_ip, observer_port),
            encryption_key,
            &game_id.to_string(),
            &server,
        ])
        .current_dir(&game_dir)
        .spawn()
        .map_err(|e| format!("启动游戏客户端失败: {}", e))?;

    Ok(format!("观战启动成功（CMD 方式），目标: {}", name))
}
