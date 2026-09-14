use tauri::{AppHandle, Manager};

use crate::lcu::client::lcu_request;

use super::HonorBallot;

pub(super) fn spawn_auto_honor(app_handle: AppHandle, ballot: HonorBallot) {
    crate::spawn_log_panic(async move {
        // 过滤掉人机；无 puuid 的队友直接跳过，避免 unwrap panic
        let allies: Vec<String> = ballot
            .eligible_allies
            .iter()
            .filter(|p| !p.bot_player)
            .filter_map(|p| p.puuid.clone())
            .collect();

        if allies.is_empty() {
            log::debug!("荣誉投票：没有可点赞的队友，跳过");
            return;
        }

        // 最多投 votePool.votes 票（默认 1）
        let votes = ballot.vote_pool.map(|p| p.votes).unwrap_or(1).max(1) as usize;
        let count = allies.len().min(votes);

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        for p in allies.iter().take(count) {
            let body = serde_json::json!({
                "honorType": "HEART",
                "recipientPuuid": p,
            });
            if let Err(e) = lcu_request(
                app_state,
                "POST",
                "/lol-honor-v2/v1/honor-player",
                Some(body),
            )
            .await
            {
                log::warn!("自动点赞失败: {}", e);
            }
        }

        // 提交投票
        if let Err(e) = lcu_request(app_state, "POST", "/lol-honor-v2/v1/ballot", None).await {
            log::warn!("提交荣誉投票失败: {}", e);
        }
        log::info!("自动荣誉点赞完成，共 {} 票", count);
    });
}
