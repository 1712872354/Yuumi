use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

use crate::config::FunctionsConfig;
use std::sync::Arc;

mod flows;
mod helpers;
mod honor;
mod post_game;

use flows::{
    spawn_aram_team_side, spawn_auto_accept, spawn_auto_play_again, spawn_handle_invitations,
    try_create_default_lobby,
};
use helpers::get_current_phase;
pub use helpers::lcu_post;
use honor::spawn_auto_honor;
use post_game::{
    spawn_cache_current_game, spawn_radar_check, spawn_record_encountered_players,
    spawn_tag_reminder,
};

/// 创建预设大厅后 LCU 会瞬时闪回 None（Lobby→None 抖动）的防抖窗口。
/// 该窗口内出现的 None 视为抖动，不重置建厅状态，避免重复建厅把玩家踢出小队。
const LOBBY_FLICKER_WINDOW: std::time::Duration = std::time::Duration::from_millis(2000);

/// 自动创建大厅的共享状态（建厅重试在后台任务中执行，需跨任务共享标志）
#[derive(Default)]
struct LobbyState {
    created: bool,
    last_create: Option<std::time::Instant>,
}

type LobbyStateHandle = Arc<std::sync::Mutex<LobbyState>>;

/// ReadyCheck 玩家是否已响应（Accepted/Declined），已响应则不再自动接受。
fn is_ready_check_already_responded(player_response: Option<&str>) -> bool {
    matches!(player_response, Some("Accepted") | Some("Declined"))
}

/// 「再来一局」阶段 → 延迟毫秒；非结算阶段返回 None。
fn play_again_delay_ms(phase: &str) -> Option<u64> {
    match phase {
        "WaitingForStats" => Some(10_000),
        "PreEndOfGame" => Some(3_250),
        "EndOfGame" => Some(1_575),
        _ => None,
    }
}

/// 进入 None 时是否应重置建厅标志。
/// 建厅成功后 LCU 会闪回 None（Lobby→None 抖动），防抖窗口内不应重置，否则重复建厅会踢出小队。
fn should_reset_lobby_on_none(within_flicker_window: bool) -> bool {
    !within_flicker_window
}

/// 是否应在阶段变化时启动自动接受匹配后台任务。
fn should_spawn_auto_accept_on_phase(phase: &str, enabled: bool, already_accepted: bool) -> bool {
    phase == "ReadyCheck" && enabled && !already_accepted
}

// ─── 游戏流程事件 ───

#[derive(Debug, Clone)]
pub enum GameflowEvent {
    PhaseChanged(String),
    ReadyCheck(ReadyCheckData),
    ResetLobbyState,
    /// 对局结束荣誉投票（/lol-honor-v2/v1/ballot）
    HonorBallot(HonorBallot),
    /// 收到的游戏邀请列表（/lol-lobby/v2/received-invitations）
    ReceivedInvitations(Vec<ReceivedInvitation>),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheckData {
    pub state: Option<String>,
    pub player_response: Option<String>,
}

/// 荣誉投票信息
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorBallot {
    pub eligible_allies: Vec<HonorEligiblePlayer>,
    pub eligible_opponents: Vec<HonorEligiblePlayer>,
    pub vote_pool: Option<HonorVotePool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorEligiblePlayer {
    #[serde(default)]
    pub bot_player: bool,
    #[serde(default)]
    pub puuid: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorVotePool {
    #[serde(default)]
    pub votes: i32,
}

/// 收到的游戏邀请
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedInvitation {
    #[serde(default)]
    pub invitation_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub can_accept_invitation: Option<bool>,
}

/// 启动游戏流程自动化后台任务。
/// 处理：自动接受匹配、自动重连、自动创建大厅、对局结束上传。
pub fn start(
    app_handle: AppHandle,
    mut rx: mpsc::Receiver<GameflowEvent>,
    upload_trigger: crate::upload::UploadTrigger,
) {
    crate::spawn_log_panic(async move {
        let mut last_phase = get_current_phase(&app_handle).await.unwrap_or_default();
        let lobby_state: LobbyStateHandle = Arc::default();
        let mut upload_trigger = upload_trigger;
        let mut ready_check_accepted = false; // 跟踪是否已接受匹配
        let mut honored = false; // 跟踪当前对局是否已自动荣誉点赞（防 WS 重复推送刷屏）

        while let Some(event) = rx.recv().await {
            let cfg = {
                let state = app_handle.state::<crate::AppState>();
                let lock = state.config.read().await;
                lock.functions.clone()
            };

            match event {
                GameflowEvent::PhaseChanged(phase) => {
                    // 阶段变化时重置接受标记
                    if phase != "ReadyCheck" {
                        ready_check_accepted = false;
                    }
                    // 进入新对局时重置荣誉点赞标记
                    if phase == "InProgress" {
                        honored = false;
                    }
                    handle_phase_change(
                        &app_handle,
                        &phase,
                        &cfg,
                        &lobby_state,
                        &mut last_phase,
                        &mut upload_trigger,
                    )
                    .await;

                    // 进入 ReadyCheck 阶段时触发自动接受匹配后台任务
                    if should_spawn_auto_accept_on_phase(
                        &phase,
                        cfg.enable_auto_accept_matching,
                        ready_check_accepted,
                    ) {
                        ready_check_accepted = true;
                        spawn_auto_accept(app_handle.clone(), cfg.auto_accept_matching_delay);
                    }
                }
                GameflowEvent::ReadyCheck(data) => {
                    if last_phase == "ReadyCheck"
                        && cfg.enable_auto_accept_matching
                        && !ready_check_accepted
                    {
                        let already_responded =
                            is_ready_check_already_responded(data.player_response.as_deref());
                        if already_responded {
                            log::debug!("收到 ReadyCheck 事件，但玩家已响应，标记为已处理");
                            ready_check_accepted = true;
                        } else {
                            ready_check_accepted = true;
                            spawn_auto_accept(app_handle.clone(), cfg.auto_accept_matching_delay);
                        }
                    }
                }
                GameflowEvent::ResetLobbyState => {
                    log::info!("收到重置大厅创建状态指令，重置为 false");
                    {
                        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.created = false;
                    }
                    if last_phase == "None" && cfg.enable_auto_create_lobby {
                        try_create_default_lobby(app_handle.clone(), &cfg, lobby_state.clone());
                    }
                }
                GameflowEvent::HonorBallot(ballot) => {
                    if cfg.enable_auto_honor && !honored {
                        honored = true;
                        spawn_auto_honor(app_handle.clone(), ballot);
                    }
                }
                GameflowEvent::ReceivedInvitations(invitations) => {
                    if cfg.enable_auto_handle_invite {
                        spawn_handle_invitations(app_handle.clone(), invitations);
                    }
                }
            }
        }
    });
}

/// 游戏阶段变化处理
async fn handle_phase_change(
    app_handle: &AppHandle,
    phase: &str,
    cfg: &FunctionsConfig,
    lobby_state: &LobbyStateHandle,
    last_phase: &mut String,
    upload_trigger: &mut crate::upload::UploadTrigger,
) {
    // debug 日志级别下高频阶段会刷屏，仅 Info 记录非 Lobby/None 的关键变化
    if phase != "Lobby" && phase != "None" {
        log::info!("游戏阶段: {}", phase);
    } else {
        log::debug!("游戏阶段: {}", phase);
    }

    // 进入对局阶段时开启自动截图事件监听，退出对局阶段时关闭
    if phase == "InProgress" {
        super::auto_screenshot::set_in_game(true);
        // 缓存本局信息，供对局结束自动记录相遇玩家
        spawn_cache_current_game(app_handle.clone());
    } else {
        super::auto_screenshot::set_in_game(false);
    }

    // 进入 "None" 空闲状态时重置大厅创建标志（允许 WS 重连后重新创建）。
    // 但创建预设大厅成功后 LCU 会在极短时间内闪回一次 None（Lobby→None 抖动），
    // 若此时也重置标志会立刻再次建厅，重复 POST 会把玩家踢出小队（"你已被移出小队"）。
    // 因此距上次建厅不足防抖窗口内出现的 None 视为抖动，跳过重置。
    if phase == "None" {
        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
        let within_flicker = lobby
            .last_create
            .map(|t| t.elapsed() < LOBBY_FLICKER_WINDOW)
            .unwrap_or(false);
        if should_reset_lobby_on_none(within_flicker) {
            lobby.created = false;
        } else {
            log::debug!("忽略 Lobby→None 抖动（距上次建厅不足防抖窗口），保留建厅状态");
        }
    }
    *last_phase = phase.to_string();

    // 每当游戏阶段变化时，通过 AtomicBool 标记 BP agent 重置状态
    // （不经过通道，无阻塞、无丢失）
    {
        let state = app_handle.state::<crate::AppState>();
        state
            .agents
            .bp_reset_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    // 空闲状态 → 自动创建预设大厅（重试循环在后台任务中执行，不阻塞事件循环）
    if phase == "None" && cfg.enable_auto_create_lobby {
        try_create_default_lobby(app_handle.clone(), cfg, lobby_state.clone());
    }

    // 游戏进行中 → 自动重连（指数退避，最多 5 次）
    if phase == "InProgress" && cfg.enable_auto_reconnect {
        log::info!("检测到游戏进行中，尝试自动重连...");
        for attempt in 1..=5 {
            if lcu_post(app_handle, "/lol-gameflow/v1/reconnect").await {
                log::info!("自动重连成功");
                break;
            }
            if attempt < 5 {
                let delay = Duration::from_millis(500 * (1 << (attempt - 1)));
                log::info!(
                    "重连失败，{}s 后重试（第 {}/5 次）",
                    delay.as_secs_f32(),
                    attempt + 1
                );
                sleep(delay).await;
            }
        }
    }

    // 再来一局：进入结算相关阶段后，延迟触发"再来一局"
    if cfg.enable_auto_play_again {
        if let Some(delay_ms) = play_again_delay_ms(phase) {
            spawn_auto_play_again(app_handle.clone(), Duration::from_millis(delay_ms));
        }
    }

    // 对局结束 → 自动记录相遇玩家（缓存消费一次，PreEndOfGame/EndOfGame 幂等）
    if matches!(phase, "EndOfGame" | "PreEndOfGame") {
        spawn_record_encountered_players(app_handle.clone());
    }

    // ARAM 换边报边：进入选人阶段后检测并播报我方队伍边
    if phase == "ChampSelect" && cfg.enable_auto_aram_team_side {
        spawn_aram_team_side(app_handle.clone(), cfg.aram_team_side_visible_to_team);
    }

    // 选人阶段 → 对带标记的玩家发送聊天提醒
    if phase == "ChampSelect" && cfg.enable_auto_tag_reminder {
        spawn_tag_reminder(app_handle.clone());
    }

    // 选人阶段 → 对局雷达：提示已知（有标签）玩家
    if phase == "ChampSelect" {
        spawn_radar_check(app_handle.clone());
    }

    // 状态转换检测 → 上传队列（包含延迟 2 秒 + 去重）
    upload_trigger.on_phase_change(phase, app_handle).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_check_already_responded() {
        assert!(is_ready_check_already_responded(Some("Accepted")));
        assert!(is_ready_check_already_responded(Some("Declined")));
        assert!(!is_ready_check_already_responded(Some("None")));
        assert!(!is_ready_check_already_responded(Some("")));
        assert!(!is_ready_check_already_responded(None));
    }

    #[test]
    fn play_again_delays_by_phase() {
        assert_eq!(play_again_delay_ms("WaitingForStats"), Some(10_000));
        assert_eq!(play_again_delay_ms("PreEndOfGame"), Some(3_250));
        assert_eq!(play_again_delay_ms("EndOfGame"), Some(1_575));
        assert_eq!(play_again_delay_ms("InProgress"), None);
        assert_eq!(play_again_delay_ms("None"), None);
        assert_eq!(play_again_delay_ms("Lobby"), None);
    }

    #[test]
    fn lobby_none_resets_only_outside_flicker_window() {
        assert!(should_reset_lobby_on_none(false));
        assert!(!should_reset_lobby_on_none(true));
    }

    #[test]
    fn auto_accept_only_on_ready_check_when_enabled_and_not_accepted() {
        assert!(should_spawn_auto_accept_on_phase("ReadyCheck", true, false));
        assert!(!should_spawn_auto_accept_on_phase("ReadyCheck", true, true));
        assert!(!should_spawn_auto_accept_on_phase(
            "ReadyCheck",
            false,
            false
        ));
        assert!(!should_spawn_auto_accept_on_phase("Lobby", true, false));
        assert!(!should_spawn_auto_accept_on_phase("None", true, false));
    }
}
