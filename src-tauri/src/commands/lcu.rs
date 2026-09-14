use crate::lcu::game_data::CherryAugmentDetail;
use crate::AppState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuConnectionDetails {
    pub pid: u32,
    pub port: u16,
    pub server: Option<String>,
}

/// 获取当前 LCU 连接信息（PID、端口、大区）
#[tauri::command]
pub async fn get_lcu_connection_info(
    app_state: tauri::State<'_, AppState>,
) -> Result<Option<LcuConnectionDetails>, String> {
    let lock = app_state.lcu.client.read().await;
    match lock.as_ref() {
        Some(client) => Ok(Some(LcuConnectionDetails {
            pid: client.pid,
            port: client.port,
            server: client.server.clone(),
        })),
        None => Ok(None),
    }
}

/// 获取选人阶段所在队伍（蓝色方/红色方）
#[tauri::command]
pub async fn get_map_side(app_state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨最长约 2.4s 的重试循环持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu.client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let auth = crate::build_auth_header(&token);
    let base = format!("https://127.0.0.1:{}", port);

    // 方法1: 从 pin-drop-notification 获取 mapSide
    // 重试最多 5 次因为选人会话初始化可能稍有延迟
    let map_side_url = format!("{}/lol-champ-select/v1/pin-drop-notification", base);
    for i in 0..5 {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        }
        match http_client
            .get(&map_side_url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(side) = data.get("mapSide").and_then(|v| v.as_str()) {
                        if !side.is_empty() {
                            log::info!("获取队伍信息成功 (pin-drop): {}", side);
                            return Ok(Some(side.to_string()));
                        }
                    }
                }
            }
            Ok(resp) => log::warn!("pin-drop-notification 返回 HTTP {}", resp.status()),
            Err(e) => log::warn!("pin-drop-notification 请求失败: {}", e),
        }
    }

    // 方法2: 读取选人会话来推断队伍
    // 如果 myTeam 的 `cellId` 小的一方为蓝色方
    let session_url = format!("{}/lol-champ-select/v1/session", base);
    match http_client
        .get(&session_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let _cell_id = data
                    .get("localPlayerCellId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if let Some(my_team) = data.get("myTeam").and_then(|v| v.as_array()) {
                    // 检查 myTeam 中最小 cellId 来判断哪一侧
                    let min_cell = my_team
                        .iter()
                        .filter_map(|p| p.get("cellId").and_then(|c| c.as_i64()))
                        .min()
                        .unwrap_or(0);
                    let max_cell = my_team
                        .iter()
                        .filter_map(|p| p.get("cellId").and_then(|c| c.as_i64()))
                        .max()
                        .unwrap_or(0);
                    // 在 5v5 中，cellId 范围 0-4 = 蓝色方，5-9 = 红色方
                    let side = if min_cell < 5 && max_cell < 5 {
                        "blue"
                    } else if min_cell >= 5 {
                        "red"
                    } else {
                        // 无法从 cellId 确定，尝试从已用的英雄 ID 推断
                        return Ok(None);
                    };
                    log::info!(
                        "获取队伍信息成功 (session cellId): {}, min={}, max={}",
                        side,
                        min_cell,
                        max_cell
                    );
                    return Ok(Some(side.to_string()));
                }
            }
        }
        _ => {}
    }

    log::warn!("无法确定队伍信息");
    Ok(None)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDataAssetsDisplay {
    pub items: std::collections::HashMap<i32, String>,
    pub spells: std::collections::HashMap<i32, String>,
    pub runes: std::collections::HashMap<i32, String>,
    pub augments: std::collections::HashMap<i32, CherryAugmentDetail>,
}

/// 获取 LCU 预加载的静态资源映射 (ID -> iconPath)
#[tauri::command]
pub async fn get_game_data_assets(
    app_state: tauri::State<'_, AppState>,
) -> Result<GameDataAssetsDisplay, String> {
    let gd = app_state.lcu.game_data.read().await;
    Ok(GameDataAssetsDisplay {
        items: gd.items.clone(),
        spells: gd.spells.clone(),
        runes: gd.runes.clone(),
        augments: gd.augments.clone(),
    })
}

/// 获取本局大乱斗板凳席我曾拥有过的英雄列表（由 ws.rs 持续写入，悬浮窗挂载时主动拉取）
#[tauri::command]
pub fn get_bench_my_champions(app_state: tauri::State<'_, AppState>) -> Vec<i64> {
    app_state
        .bench
        .my_champions
        .lock()
        .map(|list| list.clone())
        .unwrap_or_default()
}

/// 进行中对局的双方玩家（从 gameflow session 解析，供 GameInfo 在前端 session 残缺时兜底）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGamePlayer {
    pub summoner_id: i64,
    pub puuid: String,
    pub summoner_name: String,
    pub game_name: String,
    pub tag_line: String,
    pub champion_id: i32,
    pub profile_icon_id: i32,
    pub team: String, // "my" | "their"
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGameTeams {
    pub game_id: Option<i64>,
    pub my_team: Vec<LiveGamePlayer>,
    pub their_team: Vec<LiveGamePlayer>,
}

fn parse_live_players(
    arr: &[serde_json::Value],
) -> Vec<(i64, String, String, String, String, i32, i32)> {
    arr.iter()
        .filter_map(|p| {
            let summoner_id = p.get("summonerId").and_then(|v| v.as_i64()).unwrap_or(0);
            let puuid = p
                .get("puuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let game_name = p
                .get("gameName")
                .or_else(|| p.get("displayName"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tag_line = p
                .get("tagLine")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let summoner_name = p
                .get("summonerName")
                .or_else(|| p.get("displayName"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let champion_id = p
                .get("championId")
                .or_else(|| p.get("championId"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let profile_icon_id =
                p.get("profileIconId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            if summoner_id == 0
                && puuid.is_empty()
                && game_name.is_empty()
                && summoner_name.is_empty()
            {
                return None;
            }
            Some((
                summoner_id,
                puuid,
                game_name,
                tag_line,
                summoner_name,
                champion_id,
                profile_icon_id,
            ))
        })
        .collect()
}

/// 从 LCU gameflow session 拉取双方玩家；按当前召唤师归属 my/their
#[tauri::command]
pub async fn get_live_game_teams(
    app_state: tauri::State<'_, AppState>,
) -> Result<LiveGameTeams, String> {
    use crate::lcu::client::lcu_request;

    let session = lcu_request(app_state.inner(), "GET", "/lol-gameflow/v1/session", None).await?;
    let game_data = session
        .get("gameData")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let game_id = game_data.get("gameId").and_then(|v| v.as_i64());
    let team_one = game_data
        .get("teamOne")
        .and_then(|v| v.as_array())
        .map(|a| parse_live_players(a))
        .unwrap_or_default();
    let team_two = game_data
        .get("teamTwo")
        .and_then(|v| v.as_array())
        .map(|a| parse_live_players(a))
        .unwrap_or_default();

    let self_info = lcu_request(
        app_state.inner(),
        "GET",
        "/lol-summoner/v1/current-summoner",
        None,
    )
    .await
    .ok();
    let self_sid = self_info
        .as_ref()
        .and_then(|v| v.get("summonerId").and_then(|s| s.as_i64()))
        .unwrap_or(0);
    let self_puuid = self_info
        .as_ref()
        .and_then(|v| v.get("puuid").and_then(|s| s.as_str()))
        .unwrap_or("")
        .to_string();

    let contains_self = |players: &[(i64, String, String, String, String, i32, i32)]| {
        players.iter().any(|(sid, puuid, ..)| {
            (*sid > 0 && *sid == self_sid) || (!self_puuid.is_empty() && puuid == &self_puuid)
        })
    };

    let (my_raw, their_raw) = if contains_self(&team_one) {
        (team_one, team_two)
    } else if contains_self(&team_two) {
        (team_two, team_one)
    } else {
        // 未识别到自己：teamOne 按我方处理（与前端既有兜底一致）
        (team_one, team_two)
    };

    let to_players = |raw: Vec<(i64, String, String, String, String, i32, i32)>, team: &str| {
        raw.into_iter()
            .map(
                |(
                    summoner_id,
                    puuid,
                    game_name,
                    tag_line,
                    summoner_name,
                    champion_id,
                    profile_icon_id,
                )| {
                    LiveGamePlayer {
                        summoner_id,
                        puuid,
                        summoner_name,
                        game_name,
                        tag_line,
                        champion_id,
                        profile_icon_id,
                        team: team.to_string(),
                    }
                },
            )
            .collect::<Vec<_>>()
    };

    let my_team = to_players(my_raw, "my");
    let their_team = to_players(their_raw, "their");
    log::info!(
        "[LiveTeams] gameId={:?} my={} their={}",
        game_id,
        my_team.len(),
        their_team.len()
    );
    Ok(LiveGameTeams {
        game_id,
        my_team,
        their_team,
    })
}
