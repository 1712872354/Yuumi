use tauri::State;

use crate::build_auth_header;
use crate::AppState;

use super::{LcuMatchGame, LcuMatchHistoryResponse, MatchDisplay};

/// 获取战绩列表（清洗后）
#[tauri::command]
pub async fn get_match_history(
    puuid: String,
    beg_index: Option<u32>,
    end_index: Option<u32>,
    app_state: State<'_, AppState>,
) -> Result<Vec<MatchDisplay>, String> {
    // 尽早释放读锁，避免 HTTP 请求/资源等待期间阻塞 monitor 重连
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client) = (lcu.port, lcu.token, lcu.http_client);

    let mut url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches",
        port, puuid
    );

    let mut params = Vec::new();
    if let Some(b) = beg_index {
        params.push(format!("begIndex={}", b));
    }
    if let Some(e) = end_index {
        params.push(format!("endIndex={}", e));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let auth = build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取战绩失败: HTTP {}", resp.status()));
    }

    let history: LcuMatchHistoryResponse = resp.json().await.map_err(|e| e.to_string())?;

    // 如果资源尚未加载完成，且 LCU 已连接，进行等待以防止解析出来的图片/装备路径为空（最多等 5 秒）
    crate::lcu::client::wait_for_game_data(app_state.inner()).await;

    // 直接持读锁解析（to_display 为纯内存转换，无 await），避免每次全量深克隆 GameDataAssets
    let assets = app_state.lcu.game_data.read().await;
    let displays: Vec<MatchDisplay> = history
        .games
        .games
        .iter()
        .filter_map(|g| g.to_display(&assets))
        .collect();

    Ok(displays)
}

/// 通过 SGP 接口获取战绩列表（支持分页，仅腾讯国服可用）
/// 类似 getSummonerGamesByPuuidViaSGP
#[tauri::command]
pub async fn get_match_history_sgp(
    puuid: String,
    beg_index: u32,
    end_index: u32,
    app_state: State<'_, AppState>,
) -> Result<Vec<MatchDisplay>, String> {
    // 尽早释放读锁，避免跨 SGP token 获取 + 15s 超时请求持有锁阻塞 monitor 重连写锁
    let lcu = app_state.lcu_params().await?;
    let (port, token, server) = (lcu.port, lcu.token, lcu.server);

    // 仅腾讯国服支持 SGP
    let Some(server) = server else {
        log::info!("无法获取服务器信息，跳过 SGP 战绩获取");
        return Ok(Vec::new());
    };
    let server_lower = server.to_lowercase();
    if !crate::lcu::sgp::is_tencent_server(&server_lower) {
        log::info!("非腾讯国服 ({})，跳过 SGP 战绩获取", server);
        return Ok(Vec::new());
    }

    let auth = build_auth_header(&token);

    // ── 1. 获取 SGP accessToken（30 分钟缓存复用）与共享客户端 ──
    let sgp_token = crate::lcu::sgp::get_sgp_token(port, &auth).await?;

    // ── 2. 构建 SGP base URL 与客户端 ──
    let sgp_base = crate::lcu::sgp::sgp_base_url(&server_lower);
    let sgp_client = crate::lcu::sgp::get_sgp_client();

    // ── 3. 请求 SGP 战绩接口（若 401 自动强制刷新 token 重试一次） ──
    if end_index < beg_index {
        return Err("参数错误: end_index 不能小于 beg_index".to_string());
    }
    let count = end_index - beg_index + 1;
    let sgp_url = format!(
        "{}/match-history-query/v1/products/lol/player/{}/SUMMARY",
        sgp_base, puuid
    );

    let mut current_token = sgp_token;
    let mut sgp_resp = sgp_client
        .get(&sgp_url)
        .header("Authorization", format!("Bearer {}", current_token))
        .query(&[
            ("startIndex", &beg_index.to_string()),
            ("count", &count.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("SGP 战绩请求失败: {}", e))?;

    // 遇到 401 时强制刷新 token 并重试一次
    if sgp_resp.status().as_u16() == 401 {
        log::warn!("SGP 战绩返回 401 未授权，尝试强制刷新 accessToken 重试");
        if let Ok(refreshed_token) = crate::lcu::sgp::get_sgp_token_force_refresh(port, &auth).await
        {
            current_token = refreshed_token;
            sgp_resp = sgp_client
                .get(&sgp_url)
                .header("Authorization", format!("Bearer {}", current_token))
                .query(&[
                    ("startIndex", &beg_index.to_string()),
                    ("count", &count.to_string()),
                ])
                .send()
                .await
                .map_err(|e| format!("SGP 战绩重试请求失败: {}", e))?;
        }
    }

    if !sgp_resp.status().is_success() {
        return Err(format!(
            "SGP 战绩返回错误: HTTP {}",
            sgp_resp.status().as_u16()
        ));
    }

    let sgp_data: serde_json::Value = sgp_resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP 响应失败: {}", e))?;

    // ── 4. 解析 SGP 返回的对局数据 ──
    // SGP 返回格式: { "games": { "gameCount": N, "games": [{ "json": {...} }] } }
    // 或直接 { "games": [...] }
    let games = sgp_data
        .get("games")
        .and_then(|g| {
            if let Some(arr) = g.as_array() {
                Some(arr.clone())
            } else if let Some(inner) = g.get("games") {
                inner.as_array().cloned()
            } else {
                None
            }
        })
        .unwrap_or_default();

    if games.is_empty() {
        return Ok(Vec::new());
    }

    // 等待游戏资源加载完成
    crate::lcu::client::wait_for_game_data(app_state.inner()).await;

    let assets = app_state.lcu.game_data.read().await;
    let mut displays = Vec::new();

    for game_val in &games {
        // SGP 的游戏数据可能在 json 字段里
        let g = game_val.get("json").unwrap_or(game_val);

        let Some(game) = sgp_game_to_lcu_match(g, &puuid) else {
            continue;
        };
        if let Some(display) = game.to_display(&assets) {
            displays.push(display);
        }
    }

    Ok(displays)
}

/// 合并两份战绩：同 gameId 时 SGP 覆盖 LCU（更新更及时），按 timeStamp 倒序截断到 limit。
pub(crate) fn merge_match_displays(
    lcu: Vec<MatchDisplay>,
    sgp: Vec<MatchDisplay>,
    limit: usize,
) -> Vec<MatchDisplay> {
    let mut map = std::collections::HashMap::new();
    for m in lcu {
        map.insert(m.game_id, m);
    }
    for m in sgp {
        map.insert(m.game_id, m);
    }
    let mut list: Vec<MatchDisplay> = map.into_values().collect();
    list.sort_by_key(|a| std::cmp::Reverse(a.time_stamp));
    list.truncate(limit);
    list
}

/// 智能获取战绩：内部并发拉取 LCU + 首屏 SGP 并按 gameId 合并。
/// 前端只需这一条命令；非首屏不拉 SGP（除非 force_sgp），减少请求。
#[tauri::command]
pub async fn get_match_history_merged(
    puuid: String,
    beg_index: u32,
    end_index: u32,
    force_sgp: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<Vec<MatchDisplay>, String> {
    let is_home_page = beg_index == 0;
    let want_sgp = is_home_page || force_sgp.unwrap_or(false);
    let limit = (end_index.saturating_sub(beg_index) + 1) as usize;

    let lcu_fut = get_match_history(
        puuid.clone(),
        Some(beg_index),
        Some(end_index),
        app_state.clone(),
    );
    if !want_sgp {
        let lcu = lcu_fut.await.unwrap_or_else(|e| {
            log::debug!("[get_match_history_merged] LCU 战绩失败: {}", e);
            Vec::new()
        });
        return Ok(lcu);
    }

    let sgp_count = limit.saturating_sub(1).min(9) as u32;
    let sgp_fut = get_match_history_sgp(puuid, beg_index, beg_index + sgp_count, app_state);

    let (lcu_res, sgp_res) = tokio::join!(lcu_fut, sgp_fut);
    let lcu = lcu_res.unwrap_or_else(|e| {
        log::debug!("[get_match_history_merged] LCU 战绩失败: {}", e);
        Vec::new()
    });
    let sgp = sgp_res.unwrap_or_else(|e| {
        log::debug!("[get_match_history_merged] SGP 战绩失败: {}", e);
        Vec::new()
    });

    if sgp.is_empty() {
        return Ok(lcu);
    }
    if lcu.is_empty() {
        return Ok(sgp);
    }
    Ok(merge_match_displays(lcu, sgp, limit))
}

/// 将 SGP 单局 JSON 规范化为只含目标玩家的 `LcuMatchGame`，复用 `to_display` 清洗逻辑。
/// SGP 可能缺 perk0（用 perks.styles 兜底）或部分 stats 字段，结构体上的 `serde(default)` 负责兜底。
fn sgp_game_to_lcu_match(g: &serde_json::Value, puuid: &str) -> Option<LcuMatchGame> {
    let participants = g.get("participants").and_then(|v| v.as_array())?;
    let participant = participants.iter().find(|p| {
        p.get("puuid")
            .and_then(|v| v.as_str())
            .map(|p_str| p_str.eq_ignore_ascii_case(puuid))
            .unwrap_or(false)
    })?;

    // perk0 兜底：SGP 有时只给 perks.styles[].selections[].perk
    let mut participant = participant.clone();
    if let Some(obj) = participant.as_object_mut() {
        let has_perk0 = obj
            .get("stats")
            .and_then(|s| s.get("perk0"))
            .and_then(|v| v.as_i64())
            .is_some_and(|v| v != 0);
        if !has_perk0 {
            let perk0 = obj
                .get("perks")
                .and_then(|p| p.get("styles"))
                .and_then(|s| s.as_array())
                .and_then(|arr| arr.first())
                .and_then(|s0| s0.get("selections"))
                .and_then(|sel| sel.as_array())
                .and_then(|sel_arr| sel_arr.first())
                .and_then(|sel0| sel0.get("perk"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            if perk0 != 0 {
                if let Some(stats) = obj.get_mut("stats").and_then(|s| s.as_object_mut()) {
                    stats.insert("perk0".into(), serde_json::json!(perk0));
                }
            }
        }
    }

    let game_json = serde_json::json!({
        "gameId": g.get("gameId").and_then(|v| v.as_u64()).unwrap_or(0),
        "gameCreation": g.get("gameCreation").and_then(|v| v.as_u64()).unwrap_or(0),
        "gameDuration": g.get("gameDuration")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(0),
        "queueId": g.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        "mapId": g.get("mapId").and_then(|v| v.as_u64()).map(|v| v as u32),
        "participants": [participant],
    });

    serde_json::from_value::<LcuMatchGame>(game_json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn md(game_id: u64, time_stamp: u64, source: &str) -> MatchDisplay {
        MatchDisplay {
            queue_id: 420,
            game_id,
            time: source.into(),
            short_time: String::new(),
            name: String::new(),
            map: String::new(),
            duration: String::new(),
            remake: false,
            win: true,
            placement: None,
            champion_id: 0,
            spell1_id: 0,
            spell2_id: 0,
            champ_level: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            kda: "0.00".into(),
            item_ids: vec![],
            rune_id: 0,
            cs: 0,
            gold: 0,
            time_stamp,
            total_damage: 0,
            total_damage_taken: 0,
            total_heal: 0,
            vision_score: 0,
            champion_icon_url: String::new(),
            spell1_icon_url: String::new(),
            spell2_icon_url: String::new(),
            rune_icon_url: String::new(),
            item_icon_urls: vec![],
            augment_ids: vec![],
            augment_icon_urls: vec![],
            augment_names: vec![],
        }
    }

    #[test]
    fn merge_sgp_overwrites_lcu_and_sorts_desc() {
        let lcu = vec![md(1, 100, "lcu"), md(2, 300, "lcu")];
        let sgp = vec![md(2, 300, "sgp"), md(3, 200, "sgp")];
        let merged = merge_match_displays(lcu, sgp, 10);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].game_id, 2);
        assert_eq!(merged[0].time, "sgp"); // SGP 覆盖
        assert_eq!(merged[1].game_id, 3);
        assert_eq!(merged[2].game_id, 1);
    }

    #[test]
    fn merge_truncates_to_limit() {
        let lcu = vec![md(1, 100, "a"), md(2, 200, "a"), md(3, 300, "a")];
        let merged = merge_match_displays(lcu, vec![], 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].game_id, 3);
        assert_eq!(merged[1].game_id, 2);
    }

    #[test]
    fn sgp_game_matches_target_puuid_and_defaults_missing_fields() {
        let g = json!({
            "gameId": 123u64,
            "gameCreation": 1_700_000_000_000u64,
            "gameDuration": 1200u64,
            "queueId": 420,
            "participants": [
                {
                    "puuid": "OTHER",
                    "championId": 1,
                    "spell1Id": 4,
                    "spell2Id": 12,
                    "stats": { "win": true, "kills": 1, "deaths": 1, "assists": 1, "champLevel": 10 }
                },
                {
                    "puuid": "TARGET",
                    "championId": 432,
                    "spell1Id": 4,
                    "spell2Id": 12,
                    "stats": { "win": true, "kills": 10, "deaths": 2, "assists": 8, "champLevel": 18 },
                    "perks": { "styles": [{ "selections": [{ "perk": 8112 }] }] }
                }
            ]
        });

        let game = sgp_game_to_lcu_match(&g, "target").expect("should match case-insensitive");
        assert_eq!(game.game_id, 123);
        assert_eq!(game.queue_id, 420);
        assert_eq!(game.participants.len(), 1);
        let p = &game.participants[0];
        assert_eq!(p.champion_id, 432);
        assert_eq!(p.stats.kills, 10);
        assert_eq!(p.stats.perk0, 8112); // 从 perks.styles 兜底注入
        assert!(p.stats.win);
        assert_eq!(p.stats.item0, 0); // 缺失字段 default 为 0
    }

    #[test]
    fn sgp_game_returns_none_when_puuid_missing() {
        let g = json!({
            "gameId": 1,
            "gameCreation": 0,
            "gameDuration": 0,
            "queueId": 420,
            "participants": [{ "puuid": "OTHER", "stats": {} }]
        });
        assert!(sgp_game_to_lcu_match(&g, "TARGET").is_none());
    }
}
