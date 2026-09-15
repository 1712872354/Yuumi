use serde::Deserialize;
use tauri::State;

use crate::{build_auth_header, AppState};

#[derive(Deserialize)]
pub struct CreateLobbyParams {
    pub lobby_name: String,
    pub password: Option<String>,
}

/// 创建 5v5 自定义训练营房间
#[tauri::command]
pub async fn create_5v5_practice_lobby(
    params: CreateLobbyParams,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    // 尽早释放读锁，避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client) = (lcu.port, lcu.token, lcu.http_client);

    let url = format!("https://127.0.0.1:{}/lol-lobby/v1/lobby", port);
    let auth = build_auth_header(&token);

    let body = serde_json::json!({
        "customGameLobby": {
            "configuration": {
                "gameMode": "CLASSIC",
                "gameMutator": "",
                "gameServerRegion": "",
                "mapId": 11,
                "mutators": { "id": 1 },
                "spectatorPolicy": "AllAllowed",
                "teamSize": 5
            },
            "lobbyName": params.lobby_name,
            "lobbyPassword": params.password.unwrap_or_default()
        },
        "isCustom": true
    });

    let resp = http_client
        .post(&url)
        .header("Authorization", auth)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok("训练营房间已创建".to_string())
    } else {
        Err(format!("创建房间失败: HTTP {}", resp.status()))
    }
}

/// 大乱斗 (ARAM) 摇号后换回原英雄。
/// 逻辑：先 reroll，再从 bench 换回之前暂存的英雄。
#[tauri::command]
pub async fn aram_reroll_and_swap_back(app_state: State<'_, AppState>) -> Result<String, String> {
    // 尽早释放读锁，避免跨多个 HTTP await 持有锁阻塞 monitor 重连写锁
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client) = (lcu.port, lcu.token, lcu.http_client);

    let auth = build_auth_header(&token);
    let base = format!("https://127.0.0.1:{}", port);

    let sel_url = format!("{}/lol-champ-select/v1/session/my-selection", base);
    let sel_resp = http_client
        .get(&sel_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let selection: serde_json::Value = sel_resp.json().await.map_err(|e| e.to_string())?;
    let original_champion = selection
        .get("championId")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if original_champion == 0 {
        return Err("未选择英雄，无法摇号换回".to_string());
    }

    let reroll_url = format!("{}/lol-champ-select/v1/session/my-selection/reroll", base);
    let reroll_resp = http_client
        .post(&reroll_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !reroll_resp.status().is_success() {
        return Err(format!("摇号失败: HTTP {}", reroll_resp.status()));
    }

    let swap_url = format!(
        "{}/lol-champ-select/v1/session/bench/swap/{}",
        base, original_champion
    );
    let swap_resp = http_client
        .post(&swap_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if swap_resp.status().is_success() {
        Ok(format!("摇号换回成功 (原英雄: {})", original_champion))
    } else {
        Err(format!("换回失败: HTTP {}", swap_resp.status()))
    }
}
