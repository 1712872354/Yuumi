use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

use crate::lcu::client::lcu_request;

use super::helpers::{fetch_summoner_info, get_self_puuid, wait_for_champ_select_conv_id};

pub(super) fn spawn_cache_current_game(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 等待游戏会话就绪
        sleep(Duration::from_millis(1500)).await;

        let session = match lcu_request(app_state, "GET", "/lol-gameflow/v1/session", None).await {
            Ok(v) => v,
            Err(_) => return,
        };
        let Some(game_data) = session.get("gameData") else {
            return;
        };
        let Some(game_id) = game_data.get("gameId").and_then(|v| v.as_i64()) else {
            return;
        };
        let queue_id = game_data
            .get("queueId")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                game_data
                    .get("queue")
                    .and_then(|q| q.get("id"))
                    .and_then(|v| v.as_i64())
            })
            .map(|q| q as i32)
            .unwrap_or(0);

        // 云顶对局不录入路人集
        if crate::parsers::match_parser::is_tft_queue(queue_id) {
            return;
        }

        // 自己的 summonerId，用于区分队友/对手
        let self_summoner_id =
            lcu_request(app_state, "GET", "/lol-summoner/v1/current-summoner", None)
                .await
                .ok()
                .and_then(|v| v.get("summonerId").and_then(|s| s.as_i64()))
                .unwrap_or(0);

        // (summoner_id, champion_id, fallback_name, relation)
        let mut targets: Vec<(i64, i32, String, String)> = Vec::new();
        for (team_idx, team) in ["teamOne", "teamTwo"].iter().enumerate() {
            let Some(arr) = game_data.get(*team).and_then(|v| v.as_array()) else {
                continue;
            };
            for player in arr {
                let Some(summoner_id) = player.get("summonerId").and_then(|v| v.as_i64()) else {
                    continue;
                };
                let fallback_name = player
                    .get("summonerName")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("")
                    .to_string();
                let champion_id = player
                    .get("championId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                // teamOne 通常为我方；若拿到 self_summoner_id 则以本人所在队为准
                let is_my_team = if self_summoner_id > 0 {
                    // 同队成员里若有自己，则该队为 ally
                    arr.iter().any(|p| {
                        p.get("summonerId").and_then(|v| v.as_i64()) == Some(self_summoner_id)
                    })
                } else {
                    team_idx == 0
                };
                let relation = if summoner_id == self_summoner_id {
                    String::new()
                } else if is_my_team {
                    "ally".to_string()
                } else {
                    "enemy".to_string()
                };
                targets.push((summoner_id, champion_id, fallback_name, relation));
            }
        }

        // 并发拉取 10 名玩家信息（限流并发 10，避免逐个串行请求拖慢缓存）
        use futures_util::StreamExt;
        let entries: Vec<crate::saved_players::GamePlayerEntry> =
            futures_util::stream::iter(targets)
                .map(
                    |(summoner_id, champion_id, fallback_name, relation)| async move {
                        let info = fetch_summoner_info(app_state, summoner_id, "").await;
                        let info_obj = info.as_ref();
                        // 国服 Riot ID 体系下 displayName 常为空字符串，需先过滤再取 gameName，
                        // 都为空时回退到选人会话内的 summonerName
                        let summoner_name = info_obj
                            .and_then(|i| i.get("displayName"))
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                info_obj
                                    .and_then(|i| i.get("gameName"))
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                            })
                            .or(if fallback_name.is_empty() {
                                None
                            } else {
                                Some(fallback_name.as_str())
                            })
                            .unwrap_or("")
                            .to_string();
                        let puuid = info_obj
                            .and_then(|i| i.get("puuid"))
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or("")
                            .to_string();
                        let profile_icon_id = info_obj
                            .and_then(|i| i.get("profileIconId"))
                            .and_then(|v| v.as_i64())
                            .filter(|n| *n > 0)
                            .unwrap_or(0) as i32;
                        let tag_line = info_obj
                            .and_then(|i| i.get("tagLine"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        Some(crate::saved_players::GamePlayerEntry {
                            puuid,
                            summoner_name,
                            profile_icon_id,
                            tag_line,
                            champion_id,
                            relation,
                        })
                    },
                )
                .buffer_unordered(10)
                .filter_map(|x| async move { x })
                .collect()
                .await;

        let player_count = entries.len();
        if player_count == 0 {
            return;
        }

        *app_state
            .current_game_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(crate::saved_players::CurrentGameCache {
            game_id,
            queue_id,
            players: entries,
        });
        log::info!(
            "已缓存本局信息: gameId={}, queueId={}, 玩家数={}",
            game_id,
            queue_id,
            player_count
        );
    });
}

pub(super) fn spawn_record_encountered_players(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 先获取自己的 puuid，成功后再消费缓存，避免取走后因 LCU 临时不可用丢失本局记录
        let self_puuid = get_self_puuid(app_state).await;
        if self_puuid.is_empty() {
            log::warn!("对局结束：无法获取自己的 puuid，跳过相遇记录");
            return;
        }

        // 取走缓存（避免 PreEndOfGame/EndOfGame 重复记录）
        let cache = app_state
            .current_game_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let Some(cache) = cache else {
            log::debug!("对局结束：无本局信息缓存，跳过相遇记录");
            return;
        };

        let queue_type = cache.queue_id.to_string();
        let self_puuid_for_tag = self_puuid.clone();
        let recorded = crate::saved_players::record_encounters(
            app_state,
            cache.players,
            self_puuid,
            cache.game_id,
            queue_type,
        )
        .await;
        log::info!("对局结束相遇记录完成，共 {} 名玩家", recorded);

        // 自动打标（极端表现）
        spawn_auto_player_tag(app_handle.clone(), self_puuid_for_tag, cache.game_id).await;
    });
}

pub(super) async fn spawn_auto_player_tag(app_handle: AppHandle, self_puuid: String, game_id: i64) {
    // 读配置
    let enabled;
    let sensitivity;
    {
        let state = app_handle.state::<crate::AppState>();
        let guard = state.config.try_read();
        let pair = match guard {
            Ok(c) => (
                c.functions.enable_auto_player_tag,
                c.functions.auto_tag_sensitivity,
            ),
            Err(_) => (true, 1u32),
        };
        enabled = pair.0;
        sensitivity = pair.1;
    }
    if !enabled {
        return;
    }

    crate::spawn_log_panic(async move {
        // 结算数据可能稍晚才进 match-history，稍等再拉
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        let Some(game_json) = fetch_match_json_by_id(app_state, &self_puuid, game_id).await else {
            log::debug!("自动打标：未拉到对局 {} 详情，跳过", game_id);
            return;
        };

        let players = crate::auto_tag::parse_participants_from_match_json(&game_json);
        if players.len() < 4 {
            log::debug!("自动打标：参与者不足，跳过");
            return;
        }

        let tags = crate::auto_tag::evaluate_match(
            &players,
            crate::auto_tag::Sensitivity::from_u32(sensitivity),
        );
        if tags.is_empty() {
            log::info!("自动打标：本局无极端表现");
            return;
        }

        // 收集名字/英雄/关系用于 upsert
        let self_team = players
            .iter()
            .find(|p| p.puuid == self_puuid)
            .map(|p| p.team_id)
            .unwrap_or(0);
        let mut names = std::collections::HashMap::new();
        for p in &players {
            if p.puuid.is_empty() || p.puuid == self_puuid {
                continue;
            }
            let relation = if self_team != 0 && p.team_id == self_team {
                "ally"
            } else if self_team != 0 {
                "enemy"
            } else {
                ""
            };
            names.insert(
                p.puuid.clone(),
                (String::new(), p.champion_id, relation.to_string()),
            );
        }

        let applied = crate::saved_players::apply_auto_tags(
            app_state,
            self_puuid,
            game_id,
            tags.clone(),
            names,
        )
        .await;

        log::info!(
            "自动打标完成: {} 人 → {:?}",
            applied,
            tags.iter()
                .map(|t| format!("{}:{}", t.puuid, t.tag.as_str()))
                .collect::<Vec<_>>()
        );

        let _ = app_handle.emit(
            "auto-player-tagged",
            serde_json::json!({
                "gameId": game_id,
                "count": applied,
                "tags": tags.iter().map(|t| serde_json::json!({
                    "puuid": t.puuid,
                    // 事件契约存稳定 key，与 DB / 前端 helper 对齐
                    "tag": t.tag.as_key(),
                    "reason": t.reason,
                })).collect::<Vec<_>>(),
            }),
        );
    });
}

pub(super) async fn fetch_match_json_by_id(
    app_state: &crate::AppState,
    self_puuid: &str,
    game_id: i64,
) -> Option<serde_json::Value> {
    let (port, token, http_client) = {
        let lcu = app_state.lcu_params().await.ok()?;
        (lcu.port, lcu.token, lcu.http_client)
    };
    let auth = crate::build_auth_header(&token);
    let url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=5",
        port, self_puuid
    );
    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let games = body
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|v| v.as_array())?;

    for g in games {
        let raw = g.get("json").unwrap_or(g);
        let gid = raw.get("gameId").and_then(|v| v.as_i64()).unwrap_or(0);
        if gid == game_id || game_id == 0 {
            return Some(raw.clone());
        }
    }
    // 精确 gameId 未命中时跳过，避免对上一局误打标；仅 game_id==0（调用方未指定）才取最近一局
    None
}

pub(super) fn spawn_radar_check(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        // 选人数据稍等一会再拉
        sleep(Duration::from_millis(1200)).await;

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        let self_puuid = get_self_puuid(app_state).await;
        if self_puuid.is_empty() {
            return;
        }

        let session =
            match lcu_request(app_state, "GET", "/lol-champ-select/v1/session", None).await {
                Ok(v) => v,
                Err(_) => return,
            };

        let Some(my_team) = session.get("myTeam").and_then(|v| v.as_array()) else {
            return;
        };

        let mut known_puuids: Vec<String> = Vec::new();
        let mut known_names: std::collections::HashMap<String, String> = Default::default();
        for p in my_team {
            let Some(puuid) = p.get("puuid").and_then(|v| v.as_str()) else {
                continue;
            };
            if puuid.is_empty() || puuid == self_puuid {
                continue;
            }
            known_puuids.push(puuid.to_string());
            if let Some(n) = p
                .get("gameName")
                .or_else(|| p.get("displayName"))
                .or_else(|| p.get("summonerName"))
                .and_then(|v| v.as_str())
            {
                known_names.insert(puuid.to_string(), n.to_string());
            }
        }
        if known_puuids.is_empty() {
            return;
        }

        let markers = match crate::saved_players::get_saved_players_map(
            app_handle.state::<crate::AppState>(),
            self_puuid,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                log::warn!("对局雷达：读取已标记玩家失败: {}", e);
                return;
            }
        };

        let mut alerts = Vec::new();
        for puuid in known_puuids {
            let Some(m) = markers.get(&puuid) else {
                continue;
            };
            let label = m
                .auto_tag
                .clone()
                .or_else(|| m.tag.clone())
                .unwrap_or_else(|| "已标记".to_string());
            let name = known_names
                .get(&puuid)
                .cloned()
                .unwrap_or_else(|| puuid.chars().take(8).collect());
            alerts.push(serde_json::json!({
                "puuid": puuid,
                "name": name,
                "tag": label,
                "autoTag": m.auto_tag,
                "manualTag": m.tag,
                "relation": m.last_relation,
                "encounterCount": m.encounter_count,
                "listKind": m.list_kind,
                "listReason": m.list_reason,
            }));
        }

        if alerts.is_empty() {
            return;
        }

        log::info!("对局雷达：发现 {} 名已知玩家", alerts.len());
        let _ = app_handle.emit("radar-alert", serde_json::json!({ "players": alerts }));
    });
}

pub(super) fn spawn_tag_reminder(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        let self_puuid = get_self_puuid(app_state).await;
        if self_puuid.is_empty() {
            return;
        }

        let tagged =
            crate::saved_players::query_tagged_for_reminder(app_state, self_puuid.clone()).await;
        if tagged.is_empty() {
            return;
        }
        let tagged_map: std::collections::HashMap<String, String> = tagged.into_iter().collect();

        // 轮询等待选人会话就绪（最多尝试 6 次，每次 500ms）
        let mut session_opt = None;
        for _ in 0..6 {
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
            return;
        };

        let room_hint = session
            .pointer("/chatDetails/chatRoomName")
            .and_then(|v| v.as_str())
            .or_else(|| {
                session
                    .pointer("/chatDetails/multiUserChatId")
                    .and_then(|v| v.as_str())
            });

        let Some(conv_id) = wait_for_champ_select_conv_id(app_state, room_hint, 10).await else {
            log::debug!("标记玩家提醒：未找到选人聊天会话");
            return;
        };

        // 适当缓冲等待本地客户端和队友完成进入聊天室
        sleep(Duration::from_millis(2000)).await;

        // 选人聊天室只包含己方队伍，仅遍历己方即可，避免对对手做无谓查询
        // 并发拉取己方玩家信息并发送提醒（限流并发 5）
        let my_team = session
            .get("myTeam")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        use futures_util::StreamExt;
        let reminded = futures_util::stream::iter(my_team)
            .map(|player| {
                let tagged_map = &tagged_map;
                let conv_id = conv_id.clone();
                async move {
                    let mut puuid = player
                        .get("puuid")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();
                    let summoner_id = player
                        .get("summonerId")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let mut name = String::new();

                    let info = fetch_summoner_info(app_state, summoner_id, &puuid).await;
                    if let Some(info) = info {
                        if puuid.is_empty() {
                            puuid = info
                                .get("puuid")
                                .and_then(|p| p.as_str())
                                .unwrap_or("")
                                .to_string();
                        }
                        name = info
                            .get("displayName")
                            .and_then(|n| n.as_str())
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                info.get("gameName")
                                    .and_then(|n| n.as_str())
                                    .filter(|s| !s.is_empty())
                            })
                            .unwrap_or("")
                            .to_string();
                    }

                    if puuid.is_empty() {
                        return 0;
                    }

                    let Some(tag) = tagged_map.get(&puuid) else {
                        return 0;
                    };

                    if name.is_empty() {
                        name = puuid.clone();
                    }

                    let message = serde_json::json!({
                        "body": format!("[rustyuumi] 玩家 {} 已被标记：{}", name, tag),
                        "type": "celebration"
                    });
                    let path = format!("/lol-chat/v1/conversations/{}/messages", conv_id);
                    match lcu_request(app_state, "POST", &path, Some(message)).await {
                        Ok(_) => {
                            log::info!("已提醒标记玩家: {} ({})", name, tag);
                            1
                        }
                        Err(e) => {
                            log::warn!("标记玩家提醒发送失败: {}", e);
                            0
                        }
                    }
                }
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .sum::<i32>();
        if reminded == 0 {
            log::debug!("标记玩家提醒：本局没有已标记的玩家");
        }
    });
}
