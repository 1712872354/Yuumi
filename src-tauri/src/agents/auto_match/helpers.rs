use tauri::{AppHandle, Manager};

use crate::lcu::client::lcu_request;

/// Ready check 状态响应
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReadyCheckStatus {
    pub(super) player_response: Option<String>,
    pub(super) error_code: Option<String>,
}

/// 获取当前 ready check 状态
pub(super) async fn get_ready_check_status(app_handle: &AppHandle) -> Option<ReadyCheckStatus> {
    let state = app_handle.state::<crate::AppState>();
    let value = lcu_request(
        state.inner(),
        "GET",
        "/lol-matchmaking/v1/ready-check",
        None,
    )
    .await
    .ok()?;
    serde_json::from_value(value).ok()
}

/// 获取当前游戏阶段
pub(super) async fn get_current_phase(app_handle: &AppHandle) -> Option<String> {
    let state = app_handle.state::<crate::AppState>();
    let value = lcu_request(
        state.inner(),
        "GET",
        "/lol-gameflow/v1/gameflow-phase",
        None,
    )
    .await
    .ok()?;
    match value {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    }
}

/// 通用 LCU POST 请求（统一走 lcu_request，复用信号量/重试/白名单）
pub async fn lcu_post(app_handle: &AppHandle, path: &str) -> bool {
    let state = app_handle.state::<crate::AppState>();
    lcu_request(state.inner(), "POST", path, None).await.is_ok()
}

/// 异步拉取召唤师信息的辅助函数（优先通过 summonerId，失败则尝试 puuid）
pub(super) async fn fetch_summoner_info(
    app_state: &crate::AppState,
    summoner_id: i64,
    puuid: &str,
) -> Option<serde_json::Value> {
    if summoner_id > 0 {
        let path = format!("/lol-summoner/v1/summoners/{}", summoner_id);
        if let Ok(info) = lcu_request(app_state, "GET", &path, None).await {
            if info.is_object() {
                return Some(info);
            }
        }
    }
    if !puuid.is_empty() {
        let path = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
        if let Ok(info) = lcu_request(app_state, "GET", &path, None).await {
            if info.is_object() {
                return Some(info);
            }
        }
    }
    None
}

/// 获取当前召唤师 puuid（失败返回空字符串）
pub(super) async fn get_self_puuid(app_state: &crate::AppState) -> String {
    match lcu_request(app_state, "GET", "/lol-summoner/v1/current-summoner", None).await {
        Ok(v) => v
            .get("puuid")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
        Err(_) => String::new(),
    }
}

/// 轮询等待选人聊天会话 ID（从 /lol-chat/v1/conversations 中找到真正已建立的聊天室）
pub(super) async fn wait_for_champ_select_conv_id(
    app_state: &crate::AppState,
    room_hint: Option<&str>,
    max_attempts: usize,
) -> Option<String> {
    use tokio::time::{sleep, Duration};
    for attempt in 0..max_attempts {
        if attempt > 0 {
            sleep(Duration::from_millis(500)).await;
        }
        if let Ok(v) = lcu_request(app_state, "GET", "/lol-chat/v1/conversations", None).await {
            if let Some(arr) = v.as_array() {
                // 1. 如果有 room_hint，优先匹配包含该 room name / id 的会话
                if let Some(hint) = room_hint.filter(|s| !s.is_empty()) {
                    if let Some(conv) = arr.iter().find(|c| {
                        let id = c.get("id").and_then(|i| i.as_str()).unwrap_or("");
                        let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        id.contains(hint) || name == hint
                    }) {
                        if let Some(id) = conv.get("id").and_then(|i| i.as_str()) {
                            return Some(id.to_string());
                        }
                    }
                }

                // 2. 匹配 type == championSelect 或 customGame，或 id 包含 champ-select / champSelect
                if let Some(conv) = arr.iter().find(|c| {
                    let conv_type = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let conv_id = c.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    conv_type == "championSelect"
                        || conv_type == "customGame"
                        || conv_id.contains("champ-select")
                        || conv_id.contains("champSelect")
                }) {
                    if let Some(id) = conv.get("id").and_then(|i| i.as_str()) {
                        return Some(id.to_string());
                    }
                }
            }
        }
    }
    None
}
