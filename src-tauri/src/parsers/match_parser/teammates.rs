use serde::{Deserialize, Serialize};
use tauri::State;

use crate::build_auth_header;
use crate::parsers::match_parser::is_arena_queue;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTeammate {
    pub name: String,
    pub puuid: String,
    pub icon: String,
    pub total: u32,
    pub wins: u32,
    pub losses: u32,
    pub last_play_time: u64,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTeammatesResponse {
    pub puuid: String,
    pub summoners: Vec<RecentTeammate>,
}

struct TeammateInfo {
    name: String,
    puuid: String,
    icon: i32,
    win: bool,
}

struct GameTeammates {
    remake: bool,
    game_creation: u64,
    summoners: Vec<TeammateInfo>,
}

async fn fetch_game_teammates(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    game_id: u64,
    target_puuid: &str,
) -> Option<GameTeammates> {
    let detail = crate::lcu::match_detail::fetch_match_detail_json(http, base, auth, game_id)
        .await
        .ok()?;

    let game_creation = detail
        .get("gameCreation")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let queue_id = detail.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
    let participants = detail.get("participants").and_then(|v| v.as_array())?;
    let identities = detail
        .get("participantIdentities")
        .and_then(|v| v.as_array())?;

    let mut target_pid: Option<i64> = None;

    // 1. 查找目标玩家的 participantId
    for ident in identities {
        let player_data = match ident.get("player") {
            Some(p) => p,
            None => continue,
        };
        let p_puuid = player_data
            .get("puuid")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if p_puuid == target_puuid {
            target_pid = ident.get("participantId").and_then(|v| v.as_i64());
            break;
        }
    }

    let target_pid = target_pid?;

    // 2. 查找目标玩家对应的 teamId 和这一局的 remake 状态
    let mut target_team: Option<i64> = None;
    let mut remake = false;

    for p in participants {
        let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };
        if pid == target_pid {
            let stats = match p.get("stats") {
                Some(s) => s,
                None => continue,
            };
            remake = stats
                .get("gameEndedInEarlySurrender")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            target_team = {
                let stats_obj = p.get("stats").cloned().unwrap_or(serde_json::Value::Null);
                crate::lcu::match_detail::resolve_team_id(p, &stats_obj, is_arena_queue(queue_id))
                    .map(|v| v as i64)
            };
            break;
        }
    }

    let target_team = target_team?;

    // 3. 收集其他相同 teamId 的玩家作为队友
    let mut summoners = Vec::new();
    for p in participants {
        let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };
        if pid == target_pid {
            continue;
        }

        let p_team = {
            let stats_obj = p.get("stats").cloned().unwrap_or(serde_json::Value::Null);
            crate::lcu::match_detail::resolve_team_id(p, &stats_obj, is_arena_queue(queue_id))
                .map(|v| v as i64)
        };

        if p_team == Some(target_team) {
            let p_win = p
                .get("stats")
                .and_then(|s| s.get("win"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            // 在 identities 中匹配玩家信息
            for ident in identities {
                let ident_pid = match ident.get("participantId").and_then(|v| v.as_i64()) {
                    Some(id) => id,
                    None => continue,
                };
                if ident_pid == pid {
                    let player_data = match ident.get("player") {
                        Some(p) => p,
                        None => continue,
                    };
                    let game_name = player_data
                        .get("gameName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let summoner_name = player_data
                        .get("summonerName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let display_name = player_data
                        .get("displayName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let mut name = if !game_name.is_empty() {
                        game_name.to_string()
                    } else if !summoner_name.is_empty() {
                        summoner_name.to_string()
                    } else {
                        display_name.to_string()
                    };

                    let tag_line = player_data
                        .get("tagLine")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !tag_line.is_empty() && !name.is_empty() {
                        name = format!("{}#{}", name, tag_line);
                    }
                    let puuid = player_data
                        .get("puuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let icon = player_data
                        .get("profileIcon")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;

                    if !puuid.is_empty() && puuid != "00000000-0000-0000-0000-000000000000" {
                        summoners.push(TeammateInfo {
                            name,
                            puuid,
                            icon,
                            win: p_win,
                        });
                    }
                    break;
                }
            }
        }
    }

    Some(GameTeammates {
        remake,
        game_creation,
        summoners,
    })
}

#[tauri::command]
pub async fn get_recent_teammates(
    game_ids: Vec<u64>,
    puuid: String,
    app_state: State<'_, AppState>,
) -> Result<RecentTeammatesResponse, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨整个 fan-out await 持有锁阻塞 monitor 重连写锁
    let (auth, base, http) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().ok_or("LCU未连接")?;
        (
            build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
            lcu.http_client.clone(),
        )
    };

    // 复用 LCU 并发信号量，限制同时查询的对局数量，避免打满 LCU
    let semaphore = {
        let lock = app_state.lcu.api_semaphore.read().await;
        lock.clone()
    };

    // 服务端上限：防止前端一次性传入过多 game_ids 打满 LCU
    const MAX_RECENT_TEAMMATE_GAMES: usize = 20;
    let game_ids: Vec<u64> = game_ids
        .into_iter()
        .take(MAX_RECENT_TEAMMATE_GAMES)
        .collect();

    let mut handles = Vec::new();
    for game_id in game_ids {
        let auth = auth.clone();
        let base = base.clone();
        let http = http.clone();
        let target_puuid = puuid.clone();
        let semaphore = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => return None,
            };
            fetch_game_teammates(&http, &base, &auth, game_id, &target_puuid).await
        }));
    }

    let mut all_teammates = Vec::new();
    for h in handles {
        if let Ok(Some(res)) = h.await {
            all_teammates.push(res);
        }
    }

    // 查询被标记的玩家（tag 非空），标记玩家优先显示
    let tagged_map: std::collections::HashMap<String, String> =
        crate::saved_players::query_tagged_for_reminder(app_state.inner(), puuid.clone())
            .await
            .into_iter()
            .collect();

    // 统计队友
    let mut stats: std::collections::HashMap<String, RecentTeammate> =
        std::collections::HashMap::new();

    for game in all_teammates {
        for p in game.summoners {
            let entry = stats.entry(p.puuid.clone()).or_insert_with(|| {
                let icon_path = format!("/lol-game-data/assets/v1/profile-icons/{}.jpg", p.icon);
                RecentTeammate {
                    tag: tagged_map.get(&p.puuid).cloned(),
                    name: p.name,
                    puuid: p.puuid,
                    icon: icon_path,
                    total: 0,
                    wins: 0,
                    losses: 0,
                    last_play_time: game.game_creation,
                }
            });
            entry.total += 1;
            entry.last_play_time = entry.last_play_time.max(game.game_creation);
            if !game.remake {
                if p.win {
                    entry.wins += 1;
                } else {
                    entry.losses += 1;
                }
            }
        }
    }

    // 查询被标记的玩家（tag 非空），标记玩家优先显示
    let mut summoners: Vec<RecentTeammate> = stats.into_values().collect();
    // 排序：标记玩家优先，其次按 total 降序
    summoners.sort_by(|a, b| {
        let a_tagged = tagged_map.contains_key(&a.puuid);
        let b_tagged = tagged_map.contains_key(&b.puuid);
        b_tagged.cmp(&a_tagged).then_with(|| b.total.cmp(&a.total))
    });
    // 取前 5 个
    summoners.truncate(5);

    Ok(RecentTeammatesResponse { puuid, summoners })
}
