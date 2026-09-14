use std::collections::HashMap;
use tauri::State;

use crate::{build_auth_header, AppState};

use super::data::fetch_tft_data_mapping;
use super::{
    convert_lcu_icon_path, TftDataMapping, TftMatchDisplay, TftMatchSummary, TftParticipantDisplay,
    TftTraitDisplay, TftUnitDisplay,
};

fn parse_single_tft_participant(
    p: &serde_json::Value,
    pid_identity: Option<&(String, String)>,
    tft_data: &TftDataMapping,
    current_puuid: &str,
) -> TftParticipantDisplay {
    let stats = p.get("stats").unwrap_or(p);

    let (mut p_puuid, mut summoner_name) = match pid_identity {
        Some((u, n)) => (u.clone(), n.clone()),
        None => ("".to_string(), "".to_string()),
    };

    if p_puuid.is_empty() {
        p_puuid = p
            .get("puuid")
            .or_else(|| stats.get("puuid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }

    if summoner_name.is_empty() {
        // TFT 新版数据：riotIdGameName + riotIdTagline
        let riot_game_name = p
            .get("riotIdGameName")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let riot_tagline = p
            .get("riotIdTagline")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !riot_game_name.is_empty() {
            summoner_name = if riot_tagline.is_empty() {
                riot_game_name.to_string()
            } else {
                format!("{}#{}", riot_game_name, riot_tagline)
            };
        }
    }

    if summoner_name.is_empty() {
        summoner_name = p
            .get("summonerName")
            .or_else(|| p.get("gameName"))
            .or_else(|| stats.get("summonerName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }

    if summoner_name.is_empty() {
        let pid = p.get("participantId").and_then(|v| v.as_i64()).unwrap_or(0);
        if pid > 0 {
            summoner_name = format!("玩家 {}", pid);
        } else {
            summoner_name = "召唤师".to_string();
        }
    }

    let is_self = p_puuid == current_puuid;

    let placement = stats
        .get("placement")
        .or_else(|| stats.get("rank"))
        .or_else(|| p.get("placement"))
        .or_else(|| p.get("rank"))
        .and_then(|v| v.as_i64())
        .unwrap_or(8) as i32;

    let level = stats
        .get("level")
        .or_else(|| stats.get("champLevel"))
        .and_then(|v| v.as_i64())
        .unwrap_or(1) as i32;

    let gold_left = stats
        .get("goldLeft")
        .or_else(|| stats.get("gold_left"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let total_damage_to_players = stats
        .get("totalDamageToPlayers")
        .or_else(|| stats.get("total_damage_to_players"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let mut companion_icon_url = "".to_string();
    if let Some(companion) = stats.get("companion").or_else(|| p.get("companion")) {
        if let Some(item_id) = companion
            .get("skin_ID")
            .or_else(|| companion.get("item_ID"))
            .or_else(|| companion.get("skinId"))
            .and_then(|v| v.as_i64())
        {
            companion_icon_url = format!("/lol-game-data/assets/v1/profile-icons/{}.jpg", item_id);
        }
    }

    let mut units = Vec::new();
    if let Some(raw_units) = stats
        .get("units")
        .or_else(|| p.get("units"))
        .and_then(|v| v.as_array())
    {
        for u in raw_units {
            let character_id = u
                .get("character_id")
                .or_else(|| u.get("characterId"))
                .or_else(|| u.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let char_lower = character_id.to_lowercase();

            let name = tft_data
                .champions
                .get(&character_id)
                .or_else(|| tft_data.champions.get(&char_lower))
                .cloned()
                .unwrap_or_else(|| {
                    u.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&character_id)
                        .to_string()
                });

            let rarity = u.get("rarity").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let tier = u.get("tier").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

            let mut item_names = Vec::new();
            let mut item_icon_urls = Vec::new();

            if let Some(arr) = u
                .get("itemNames")
                .or_else(|| u.get("items"))
                .and_then(|v| v.as_array())
            {
                for i in arr {
                    let key = if let Some(s) = i.as_str() {
                        s.to_string()
                    } else if let Some(n) = i.as_i64() {
                        n.to_string()
                    } else {
                        continue;
                    };

                    let key_lower = key.to_lowercase();

                    let translated = tft_data
                        .item_names
                        .get(&key)
                        .or_else(|| tft_data.item_names.get(&key_lower))
                        .cloned()
                        .unwrap_or_else(|| key.clone());

                    let raw_item_icon = tft_data
                        .item_icons
                        .get(&key)
                        .or_else(|| tft_data.item_icons.get(&key_lower))
                        .cloned()
                        .unwrap_or_default();

                    let item_icon_url = convert_lcu_icon_path(&raw_item_icon);

                    item_names.push(translated);
                    if !item_icon_url.is_empty() {
                        item_icon_urls.push(item_icon_url);
                    }
                }
            }

            let raw_icon = tft_data
                .champion_icons
                .get(&character_id)
                .or_else(|| tft_data.champion_icons.get(&char_lower))
                .cloned()
                .unwrap_or_else(|| {
                    u.get("icon")
                        .or_else(|| u.get("squareIcon"))
                        .or_else(|| u.get("iconUrl"))
                        .or_else(|| u.get("iconPath"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                });

            let icon_url = convert_lcu_icon_path(&raw_icon);

            units.push(TftUnitDisplay {
                character_id,
                name,
                icon_url,
                rarity,
                tier,
                item_names,
                item_icon_urls,
            });
        }
    }

    let mut traits = Vec::new();
    if let Some(raw_traits) = stats
        .get("traits")
        .or_else(|| p.get("traits"))
        .and_then(|v| v.as_array())
    {
        for t in raw_traits {
            let trait_api_name = t
                .get("name")
                .or_else(|| t.get("trait_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let trait_lower = trait_api_name.to_lowercase();

            let name = tft_data
                .traits
                .get(&trait_api_name)
                .or_else(|| tft_data.traits.get(&trait_lower))
                .cloned()
                .unwrap_or_else(|| trait_api_name.clone());

            let num_units = t
                .get("num_units")
                .or_else(|| t.get("numUnits"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let tier_current = t
                .get("tier_current")
                .or_else(|| t.get("tierCurrent"))
                .or_else(|| t.get("style"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            if tier_current > 0 || num_units > 0 {
                let raw_trait_icon = tft_data
                    .trait_icons
                    .get(&trait_api_name)
                    .or_else(|| tft_data.trait_icons.get(&trait_lower))
                    .cloned()
                    .unwrap_or_else(|| {
                        t.get("icon")
                            .or_else(|| t.get("iconPath"))
                            .or_else(|| t.get("iconUrl"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string()
                    });

                let icon_url = convert_lcu_icon_path(&raw_trait_icon);

                traits.push(TftTraitDisplay {
                    name,
                    num_units,
                    tier_current,
                    icon_url,
                });
            }
        }
    }

    let mut augments = Vec::new();
    if let Some(raw_augments) = stats
        .get("augments")
        .or_else(|| p.get("augments"))
        .and_then(|v| v.as_array())
    {
        augments = raw_augments
            .iter()
            .filter_map(|a| {
                if let Some(s) = a.as_str() {
                    Some(s.to_string())
                } else {
                    a.as_i64().map(|n| n.to_string())
                }
            })
            .collect();
    }

    TftParticipantDisplay {
        puuid: p_puuid,
        summoner_name,
        is_self,
        placement,
        level,
        gold_left,
        total_damage_to_players,
        companion_icon_url,
        traits,
        units,
        augments,
    }
}

/// 获取云顶之弈战绩列表与汇总统计
#[tauri::command]
pub async fn get_tft_match_history(
    puuid: String,
    beg_index: Option<u32>,
    end_index: Option<u32>,
    app_state: State<'_, AppState>,
) -> Result<TftMatchSummary, String> {
    // 尽早释放读锁，避免 HTTP/CDN 请求期间阻塞 monitor 重连
    let lcu = app_state.lcu_params().await?;
    let (port, token, server, http_client) = (lcu.port, lcu.token, lcu.server, lcu.http_client);

    // 预先拉取/建立 TFT 资源与图标映射字典
    let tft_data = fetch_tft_data_mapping(None).await;

    let auth = build_auth_header(&token);

    let b = beg_index.unwrap_or(0);
    let e = end_index.unwrap_or(20);
    let count = if e >= b { e - b + 1 } else { 20 };

    let candidate_urls = vec![
        format!(
            "https://127.0.0.1:{}/lol-match-history/v1/products/tft/{}/matches?begIndex={}&endIndex={}",
            port, puuid, b, e
        ),
        format!(
            "https://127.0.0.1:{}/lol-match-history/v1/products/tft/{}/matches?beginIndex={}&endIndex={}",
            port, puuid, b, e
        ),
        format!(
            "https://127.0.0.1:{}/lol-match-history/v1/products/tft/{}/matches",
            port, puuid
        ),
    ];

    let mut raw_json: Option<serde_json::Value> = None;

    for url in candidate_urls {
        if let Ok(resp) = http_client
            .get(&url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    raw_json = Some(json);
                    break;
                }
            }
        }
    }

    // 若 LCU 本地接口未成功获取，尝试使用 SGP 接口获取
    if raw_json.is_none() {
        if let Some(server) = server.as_ref() {
            let server_lower = server.to_lowercase();
            if crate::lcu::sgp::is_tencent_server(&server_lower) {
                // SGP token 30 分钟缓存复用，共享客户端进程级复用
                if let Ok(sgp_token) = crate::lcu::sgp::get_sgp_token(port, &auth).await {
                    let sgp_base = crate::lcu::sgp::sgp_base_url(&server_lower);
                    let sgp_url = format!(
                        "{}/match-history-query/v1/products/tft/player/{}/SUMMARY",
                        sgp_base, puuid
                    );
                    if let Ok(sgp_resp) = crate::lcu::sgp::get_sgp_client()
                        .get(&sgp_url)
                        .header("Authorization", format!("Bearer {}", sgp_token))
                        .query(&[
                            ("startIndex", &b.to_string()),
                            ("count", &count.to_string()),
                        ])
                        .send()
                        .await
                    {
                        if sgp_resp.status().is_success() {
                            if let Ok(sgp_json) = sgp_resp.json::<serde_json::Value>().await {
                                raw_json = Some(sgp_json);
                            }
                        }
                    }
                }
            }
        }
    }

    let raw_json = match raw_json {
        Some(j) => j,
        None => {
            log::warn!("无法获取云顶战绩数据，返回空统计");
            return Ok(TftMatchSummary {
                total_games: 0,
                win_count: 0,
                top4_count: 0,
                top4_rate: 0.0,
                win_rate: 0.0,
                avg_placement: 0.0,
                matches: Vec::new(),
            });
        }
    };

    let games = raw_json
        .get("games")
        .and_then(|g| g.get("games").or(Some(g)))
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();

    let mut cleaned_matches = Vec::new();
    let mut win_count = 0;
    let mut top4_count = 0;
    let mut total_placement_sum = 0;

    for raw_g in &games {
        // 解包 SGP 或 LCU 结构中的 json 嵌套
        let g_obj = if let Some(j_val) = raw_g.get("json") {
            if j_val.is_string() {
                serde_json::from_str::<serde_json::Value>(j_val.as_str().unwrap())
                    .unwrap_or_else(|_| raw_g.clone())
            } else if j_val.is_object() {
                j_val.clone()
            } else {
                raw_g.clone()
            }
        } else {
            raw_g.clone()
        };

        let game_id = g_obj.get("gameId").and_then(|v| v.as_u64()).unwrap_or(0);
        let game_creation = g_obj
            .get("gameCreation")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let raw_duration = g_obj
            .get("game_length")
            .or_else(|| g_obj.get("gameDuration"))
            .or_else(|| g_obj.get("gameLength"));

        let game_duration = raw_duration
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(0);
        let queue_id = g_obj.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let queue_name = match queue_id {
            1090 => "云顶之弈(匹配)",
            1100 => "云顶之弈(排位)",
            1130 => "狂暴模式",
            1160 => "双人作战",
            _ => "云顶模式",
        }
        .to_string();

        let secs = (game_creation / 1000) as i64;
        let time_str = chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| {
                let utc8_fixed = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
                dt.with_timezone(&utc8_fixed)
                    .format("%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "01-01 00:00".to_string());

        let duration_str = format!("{:02}:{:02}", game_duration / 60, game_duration % 60);

        let raw_participants = g_obj
            .get("participants")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // participantId → (puuid, name)，用于 SR 风格数据
        let mut pid_map: HashMap<i64, (String, String)> = HashMap::new();
        // puuid → name，用于 TFT 风格数据（participants 无 participantId）
        let mut puuid_name_map: HashMap<String, String> = HashMap::new();

        if let Some(identities) = g_obj
            .get("participantIdentities")
            .and_then(|v| v.as_array())
        {
            for item in identities {
                let pid = item
                    .get("participantId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if let Some(player) = item.get("player") {
                    let player_puuid = player
                        .get("puuid")
                        .or_else(|| player.get("currentPuuid"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let game_name = player
                        .get("gameName")
                        .or_else(|| player.get("game_name"))
                        .and_then(|v| v.as_str());

                    let tag_line = player
                        .get("tagLine")
                        .or_else(|| player.get("tag_line"))
                        .and_then(|v| v.as_str());

                    let summoner_name = if let (Some(gn), Some(tl)) = (game_name, tag_line) {
                        format!("{}#{}", gn, tl)
                    } else if let Some(gn) = game_name {
                        gn.to_string()
                    } else if let Some(sn) = player
                        .get("summonerName")
                        .or_else(|| player.get("displayName"))
                        .or_else(|| player.get("name"))
                        .and_then(|v| v.as_str())
                    {
                        sn.to_string()
                    } else {
                        "".to_string()
                    };

                    if pid > 0 {
                        pid_map.insert(pid, (player_puuid.clone(), summoner_name.clone()));
                    }
                    if !player_puuid.is_empty() && !summoner_name.is_empty() {
                        puuid_name_map.insert(player_puuid, summoner_name);
                    }
                }
            }
        }

        let mut parsed_participants = Vec::new();
        for p_val in &raw_participants {
            let pid = p_val
                .get("participantId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            // 优先用 participantId 查（SR 风格），取不到再用 participant 自身 puuid 查（TFT 风格）
            let pid_identity = if pid > 0 { pid_map.get(&pid) } else { None };
            let p_puuid_for_lookup = p_val
                .get("puuid")
                .or_else(|| p_val.get("stats").and_then(|s| s.get("puuid")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let puuid_identity: Option<(String, String)> =
                if pid_identity.is_none() && !p_puuid_for_lookup.is_empty() {
                    puuid_name_map
                        .get(p_puuid_for_lookup)
                        .map(|name| (p_puuid_for_lookup.to_string(), name.clone()))
                } else {
                    None
                };
            let effective_identity = pid_identity.or(puuid_identity.as_ref());
            let parsed_p =
                parse_single_tft_participant(p_val, effective_identity, &tft_data, &puuid);
            parsed_participants.push(parsed_p);
        }

        // 按名次 (#1 ~ #8) 升序排序
        parsed_participants.sort_by_key(|p| p.placement);

        let my_p = parsed_participants
            .iter()
            .find(|p| p.is_self)
            .cloned()
            .unwrap_or_else(|| {
                parsed_participants
                    .first()
                    .cloned()
                    .unwrap_or(TftParticipantDisplay {
                        puuid: puuid.clone(),
                        summoner_name: "我".to_string(),
                        is_self: true,
                        placement: 8,
                        level: 1,
                        gold_left: 0,
                        total_damage_to_players: 0,
                        companion_icon_url: "".to_string(),
                        traits: Vec::new(),
                        units: Vec::new(),
                        augments: Vec::new(),
                    })
            });

        if my_p.placement == 1 {
            win_count += 1;
        }
        if my_p.placement <= 4 {
            top4_count += 1;
        }
        total_placement_sum += my_p.placement;

        cleaned_matches.push(TftMatchDisplay {
            game_id,
            queue_id,
            queue_name,
            game_creation,
            game_duration,
            time_str,
            duration_str,
            placement: my_p.placement,
            level: my_p.level,
            gold_left: my_p.gold_left,
            total_damage_to_players: my_p.total_damage_to_players,
            companion_icon_url: my_p.companion_icon_url,
            traits: my_p.traits,
            units: my_p.units,
            augments: my_p.augments,
            participants: parsed_participants,
        });
    }

    let total_games = cleaned_matches.len();
    let (top4_rate, win_rate, avg_placement) = if total_games > 0 {
        (
            (top4_count as f64 / total_games as f64) * 100.0,
            (win_count as f64 / total_games as f64) * 100.0,
            total_placement_sum as f64 / total_games as f64,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    Ok(TftMatchSummary {
        total_games,
        win_count,
        top4_count,
        top4_rate,
        win_rate,
        avg_placement,
        matches: cleaned_matches,
    })
}
