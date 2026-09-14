use serde::{Deserialize, Serialize};
use tauri::State;

use crate::parsers::match_parser::is_arena_queue;
use crate::{build_auth_header, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerFateInfo {
    pub fate_flag: Option<String>,
    pub recently_champion_name: Option<String>,
}

/// 前端独立调用的单个玩家宿命获取接口
#[tauri::command]
pub async fn get_player_fate_info(
    game_id: u64,
    target_puuid: String,
    current_summoner_id: u64,
    app_state: State<'_, AppState>,
) -> Result<PlayerFateInfo, String> {
    // 锁内只提取连接参数，尽早释放读锁，避免 HTTP 请求期间阻塞 monitor 重连
    let (auth, base, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().ok_or("LCU未连接")?;
        (
            build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
            lcu.http_client.clone(),
        )
    };

    let detail =
        crate::lcu::match_detail::fetch_match_detail_json(&http_client, &base, &auth, game_id)
            .await?;

    let queue_id = detail.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
    let participants = detail
        .get("participants")
        .and_then(|v| v.as_array())
        .ok_or("无法解析 participants")?;
    let identities = detail
        .get("participantIdentities")
        .and_then(|v| v.as_array())
        .ok_or("无法解析 participantIdentities")?;

    let mut current_pid: Option<i64> = None;
    let mut target_pid: Option<i64> = None;

    // 1. 查找 participantId
    for ident in identities {
        let player_data = match ident.get("player") {
            Some(p) => p,
            None => continue,
        };
        let puuid = player_data
            .get("puuid")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let summoner_id = player_data
            .get("summonerId")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let pid = match ident.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };

        if puuid == target_puuid {
            target_pid = Some(pid);
        }
        if summoner_id == current_summoner_id {
            current_pid = Some(pid);
        }
    }

    let target_pid = target_pid.ok_or("找不到目标玩家")?;

    // 查找目标玩家在这一局使用的英雄 ID
    let mut target_champion_id: Option<i32> = None;
    for p in participants {
        let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };
        if pid == target_pid {
            target_champion_id = p
                .get("championId")
                .and_then(|v| v.as_i64())
                .map(|id| id as i32);
            break;
        }
    }

    let recently_champion_name = if let Some(cid) = target_champion_id {
        let assets = app_state.lcu.game_data.read().await;
        assets.champions.get(&cid).cloned()
    } else {
        None
    };

    let fate_flag = if let Some(curr_pid) = current_pid {
        if curr_pid == target_pid {
            None
        } else {
            // 2. 查找对应的队伍 ID
            let mut current_team: Option<i64> = None;
            let mut target_team: Option<i64> = None;

            for p in participants {
                let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
                    Some(id) => id,
                    None => continue,
                };

                let stats_obj = p.get("stats").cloned().unwrap_or(serde_json::Value::Null);
                let team_val = crate::lcu::match_detail::resolve_team_id(
                    p,
                    &stats_obj,
                    is_arena_queue(queue_id),
                )
                .map(|v| v as i64);

                if pid == curr_pid {
                    current_team = team_val;
                }
                if pid == target_pid {
                    target_team = team_val;
                }
            }

            if let (Some(ct), Some(tt)) = (current_team, target_team) {
                if ct == tt {
                    Some("ally".to_string())
                } else {
                    Some("enemy".to_string())
                }
            } else {
                None
            }
        }
    } else {
        None
    };

    Ok(PlayerFateInfo {
        fate_flag,
        recently_champion_name,
    })
}
