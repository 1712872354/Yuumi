use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration};

use crate::config::FunctionsConfig;
use crate::lcu::client::lcu_request;

use super::helpers::{
    get_current_phase, get_ready_check_status, lcu_post, wait_for_champ_select_conv_id,
};
use super::{LobbyStateHandle, ReceivedInvitation};

pub(super) fn try_create_default_lobby(
    app_handle: AppHandle,
    cfg: &FunctionsConfig,
    lobby_state: LobbyStateHandle,
) {
    // 已建成或已在建厅重试中则跳过（占位防止后续事件重复启动建厅任务）
    {
        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
        if lobby.created {
            return;
        }
        lobby.created = true;
    }

    let queue_id = cfg.default_game_mode;
    crate::spawn_log_panic(async move {
        log::info!("自动创建预设大厅: queueId={}", queue_id);

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        for attempt in 0..30 {
            // 检查 LCU 是否仍然连接
            if app_state.lcu.client.read().await.as_ref().is_none() {
                log::info!("LCU 已断开，停止创建大厅");
                return;
            }

            // 检查当前阶段是否仍为 None
            if let Ok(serde_json::Value::String(phase)) =
                lcu_request(app_state, "GET", "/lol-gameflow/v1/gameflow-phase", None).await
            {
                if !matches!(
                    phase.as_str(),
                    "None" | "" | "WaitingForStats" | "PreEndOfGame"
                ) {
                    log::info!("当前阶段为 {}，跳过创建大厅", phase);
                    return;
                }
            }

            // 尝试创建大厅
            let body = serde_json::json!({ "queueId": queue_id });
            match lcu_request(app_state, "POST", "/lol-lobby/v2/lobby", Some(body)).await {
                Ok(_) => {
                    log::info!("预设大厅创建成功 (尝试 {})", attempt + 1);
                    let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                    lobby.last_create = Some(std::time::Instant::now());
                    return;
                }
                Err(e) => {
                    if e.contains("409") {
                        log::info!("创建大厅返回 409 (Conflict)，可能已在房间中，停止重试");
                        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.last_create = Some(std::time::Instant::now());
                        return;
                    }
                    log::warn!("创建大厅失败: {}，重试中...", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        log::warn!("创建预设大厅：30 次重试均失败");
    });
}

pub(super) fn spawn_auto_accept(app_handle: AppHandle, delay_secs: u32) {
    crate::spawn_log_panic(async move {
        if delay_secs > 0 {
            log::info!("将在 {} 秒后自动接受匹配...", delay_secs);
            tokio::time::sleep(std::time::Duration::from_secs(delay_secs as u64)).await;
        } else {
            log::info!("立即自动接受匹配");
        }

        // 延迟后，先检查当前游戏阶段是否仍然是 ReadyCheck
        match get_current_phase(&app_handle).await {
            Some(current_phase) => {
                if current_phase != "ReadyCheck" {
                    log::info!(
                        "延迟后当前游戏阶段为 {}，不再是 ReadyCheck，取消自动接受",
                        current_phase
                    );
                    return;
                }
            }
            None => {
                log::warn!("延迟后无法获取当前游戏阶段，取消自动接受");
                return;
            }
        }

        // 再次获取当前 ready check 状态以确认是否已被响应
        match get_ready_check_status(&app_handle).await {
            Some(status) => {
                // 检查玩家响应状态
                if let Some(ref response) = status.player_response {
                    if response == "Accepted" || response == "Declined" {
                        log::debug!("延迟后玩家已响应匹配: {}，跳过自动接受", response);
                        return;
                    }
                }
                // 检查是否有错误
                if let Some(ref error_code) = status.error_code {
                    log::warn!("延迟后 Ready check 发生错误: {}，取消自动接受", error_code);
                    return;
                }
            }
            None => {
                log::debug!("延迟后无法获取 ready check 状态，取消自动接受");
                return;
            }
        }

        log::info!("自动接受匹配");
        lcu_post(&app_handle, "/lol-matchmaking/v1/ready-check/accept").await;
    });
}

pub(super) fn spawn_handle_invitations(
    app_handle: AppHandle,
    invitations: Vec<ReceivedInvitation>,
) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        for inv in invitations {
            // 只处理待处理的邀请
            let pending = inv.state.as_deref() == Some("Pending");
            let can_accept = inv.can_accept_invitation.unwrap_or(false);
            let Some(invitation_id) = inv.invitation_id.as_deref() else {
                continue;
            };

            if !pending || !can_accept {
                continue;
            }

            let path = format!(
                "/lol-lobby/v2/received-invitations/{}/accept",
                invitation_id
            );
            match lcu_request(app_state, "POST", &path, None).await {
                Ok(_) => log::info!("已自动接受邀请 {}", invitation_id),
                Err(e) => log::warn!("自动接受邀请 {} 失败: {}", invitation_id, e),
            }
        }
    });
}

pub(super) fn spawn_auto_play_again(app_handle: AppHandle, delay: Duration) {
    crate::spawn_log_panic(async move {
        sleep(delay).await;
        log::info!("自动再来一局");
        lcu_post(&app_handle, "/lol-lobby/v2/play-again").await;
    });
}

pub(super) fn spawn_aram_team_side(app_handle: AppHandle, visible_to_team: bool) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 轮询等待选人会话就绪（最多尝试 8 次，每次 500ms）
        let mut session_opt = None;
        for _ in 0..8 {
            sleep(Duration::from_millis(500)).await;
            if let Ok(v) = lcu_request(app_state, "GET", "/lol-champ-select/v1/session", None).await
            {
                if v.is_object() {
                    session_opt = Some(v);
                    break;
                }
            }
        }

        let Some(session) = session_opt else {
            log::warn!("选人报边：获取选人会话超时");
            return;
        };

        // 优先从 pin-drop-notification 获取地图阵营
        let mut side = None;
        if let Ok(data) = lcu_request(
            app_state,
            "GET",
            "/lol-champ-select/v1/pin-drop-notification",
            None,
        )
        .await
        {
            if let Some(map_side) = data.get("mapSide").and_then(|v| v.as_str()) {
                if !map_side.is_empty() {
                    side = Some(map_side.to_lowercase());
                }
            }
        }

        // 降级使用 cellId 判断队伍（5v5 中 0-4 为蓝方，5-9 为红方）
        if side.is_none() {
            let cell_id = session
                .get("localPlayerCellId")
                .and_then(|c| c.as_i64())
                .unwrap_or(0);
            if cell_id < 5 {
                side = Some("blue".to_string());
            } else {
                side = Some("red".to_string());
            }
        }

        let Some(side) = side else {
            log::warn!("选人报边：无法判定红蓝方阵营");
            return;
        };

        let side_name = if side == "blue" {
            "蓝色方"
        } else {
            "红色方"
        };

        // 从已建立的聊天会话列表中检索会话 ID（最多等待 10 次 × 500ms）
        let room_hint = session
            .pointer("/chatDetails/chatRoomName")
            .and_then(|v| v.as_str())
            .or_else(|| {
                session
                    .pointer("/chatDetails/multiUserChatId")
                    .and_then(|v| v.as_str())
            });

        let Some(conv_id) = wait_for_champ_select_conv_id(app_state, room_hint, 10).await else {
            log::warn!("选人报边：未找到选人聊天会话");
            return;
        };

        // 适当缓冲等待本地客户端和队友完成进入聊天室（避免消息在入房初始化前被冲刷覆盖）
        sleep(Duration::from_millis(2000)).await;

        // 发送报边消息；visible_to_team 为 false 时使用 celebration 类型（本地私密广播，队友不可见）
        let message = if visible_to_team {
            serde_json::json!({
                "body": format!("[rustyuumi] 本局我方在{}", side_name),
                "type": "chat"
            })
        } else {
            serde_json::json!({
                "body": format!("[rustyuumi] 本局我方在{}", side_name),
                "type": "celebration"
            })
        };
        let path = format!("/lol-chat/v1/conversations/{}/messages", conv_id);

        // 重试最多 3 次发送，确保聊天室通道稳定
        let mut sent = false;
        for attempt in 0..3 {
            if attempt > 0 {
                sleep(Duration::from_millis(600)).await;
            }
            match lcu_request(app_state, "POST", &path, Some(message.clone())).await {
                Ok(_) => {
                    log::info!(
                        "选人报边成功：我方在{}（队友可见={}）",
                        side_name,
                        visible_to_team
                    );
                    sent = true;
                    break;
                }
                Err(e) => {
                    log::warn!("选人报边第 {} 次尝试失败: {}", attempt + 1, e);
                }
            }
        }
        if !sent {
            log::warn!("选人报边最终发送失败");
        }
    });
}
