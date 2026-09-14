use rusqlite::params;

use crate::AppState;

use super::types::GamePlayerEntry;
use super::{now, with_db};

// ─── 对局结束自动记录相遇 ───

/// 记录对局相遇：仅更新库中**已存在**玩家的 lastMetAt / 身份信息，并写入 encountered_games。
/// 不再为每局 10 人批量建行——新玩家只有手动标记或自动打标时才入库，避免库膨胀。
pub async fn record_encounters(
    state: &AppState,
    players: Vec<GamePlayerEntry>,
    self_puuid: String,
    game_id: i64,
    queue_type: String,
) -> usize {
    with_db(state, move |conn| {
        let ts = now();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut recorded = 0;

        for player in &players {
            if player.puuid.is_empty() || player.puuid == self_puuid {
                continue;
            }

            // 仅更新已存在的行；不存在则跳过（不 INSERT）
            let updated = tx
                .execute(
                    "UPDATE saved_players SET
                       summoner_name = CASE WHEN ?1 = '' THEN summoner_name ELSE ?1 END,
                       profile_icon_id = CASE WHEN ?2 = 0 THEN profile_icon_id ELSE ?2 END,
                       tag_line = CASE WHEN ?3 IS NULL OR ?3 = '' THEN tag_line ELSE ?3 END,
                       champion_id = CASE WHEN ?4 = 0 THEN champion_id ELSE ?4 END,
                       last_relation = CASE WHEN ?8 = '' THEN last_relation ELSE ?8 END,
                       update_at = ?5,
                       last_met_at = ?5
                     WHERE puuid = ?6 AND self_puuid = ?7",
                    params![
                        player.summoner_name,
                        player.profile_icon_id,
                        player.tag_line,
                        player.champion_id,
                        ts,
                        player.puuid,
                        self_puuid,
                        player.relation
                    ],
                )
                .map_err(|e| format!("更新已记录玩家失败: {}", e))?;

            if updated == 0 {
                continue;
            }

            tx.execute(
                "INSERT INTO encountered_games (game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at)
                 VALUES (?1, ?2, ?3, '', '', ?4, ?5)",
                params![game_id, player.puuid, self_puuid, queue_type, ts],
            )
            .map_err(|e| format!("记录相遇对局失败: {}", e))?;

            recorded += 1;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(recorded)
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("记录相遇玩家失败: {}", e);
        0
    })
}

/// 查询带 tag 的玩家（选人阶段聊天提醒使用），返回 (puuid, tag)
pub async fn query_tagged_for_reminder(
    state: &AppState,
    self_puuid: String,
) -> Vec<(String, String)> {
    with_db(state, move |conn| {
        let mut stmt = match conn.prepare(
            "SELECT puuid, tag FROM saved_players WHERE self_puuid = ?1 AND tag IS NOT NULL AND tag != ''",
        ) {
            Ok(s) => s,
            Err(e) => {
                log::error!("查询标记玩家失败: {}", e);
                return Ok(Vec::new());
            }
        };
        let rows = stmt.query_map(params![self_puuid], |r| Ok((r.get(0)?, r.get(1)?)));
        let out = match rows {
            Ok(iter) => iter.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
            Err(e) => {
                log::error!("查询标记玩家失败: {}", e);
                Vec::new()
            }
        };
        Ok(out)
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("查询标记玩家失败: {}", e);
        Vec::new()
    })
}

/// 应用自动打标结果：更新 auto_tag / auto_score / auto_reason，并累计历史统计。
/// 不覆盖用户手动 tag。玩家行不存在时先 upsert 一行。
pub async fn apply_auto_tags(
    state: &AppState,
    self_puuid: String,
    game_id: i64,
    tags: Vec<crate::auto_tag::AutoTagResult>,
    names: std::collections::HashMap<String, (String, i32, String)>, // puuid -> (name, champion_id, relation)
) -> usize {
    with_db(state, move |conn| {
        if tags.is_empty() {
            return Ok(0);
        }
        let ts = now();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut applied = 0;

        for t in &tags {
            if t.puuid.is_empty() || t.puuid == self_puuid {
                continue;
            }
            let (summoner_name, champion_id, relation) = names
                .get(&t.puuid)
                .cloned()
                .unwrap_or_else(|| (String::new(), 0, String::new()));

            // 确保有行
            let _ = tx.execute(
                "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, update_at, last_met_at, champion_id, last_relation)
                 VALUES (?1, ?2, '', '', NULL, ?3, 0, ?4, ?4, ?5, ?6)
                 ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
                   summoner_name = CASE WHEN excluded.summoner_name = '' THEN saved_players.summoner_name ELSE excluded.summoner_name END,
                   champion_id = CASE WHEN excluded.champion_id = 0 THEN saved_players.champion_id ELSE excluded.champion_id END,
                   last_relation = CASE WHEN excluded.last_relation = '' THEN saved_players.last_relation ELSE excluded.last_relation END,
                   update_at = excluded.update_at,
                   last_met_at = excluded.last_met_at",
                params![t.puuid, self_puuid, summoner_name, ts, champion_id, relation],
            );

            // 读取并更新历史标签统计
            let stats_json: Option<String> = tx
                .query_row(
                    "SELECT auto_tag_stats FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
                    params![t.puuid, self_puuid],
                    |r| r.get(0),
                )
                .ok()
                .flatten();

            let mut stats: std::collections::HashMap<String, i32> = stats_json
                .and_then(|s| {
                    serde_json::from_str::<std::collections::HashMap<String, i32>>(&s).ok()
                })
                .unwrap_or_default();
            let key = t.tag.as_key().to_string();
            *stats.entry(key).or_insert(0) += 1;
            let stats_str = serde_json::to_string(&stats).unwrap_or_else(|_| "{}".into());

            let _ = tx.execute(
                "UPDATE saved_players SET auto_tag = ?1, auto_score = ?2, auto_reason = ?3, auto_tag_stats = ?4, update_at = ?5
                 WHERE puuid = ?6 AND self_puuid = ?7",
                params![
                    // 存稳定 key（carry/feeder/...），展示层再本地化；勿再写中文串
                    t.tag.as_key(),
                    t.score,
                    t.reason,
                    stats_str,
                    ts,
                    t.puuid,
                    self_puuid
                ],
            );
            applied += 1;
        }

        let _ = game_id; // 预留：后续可写 auto_tag_games 明细
        tx.commit().map_err(|e| e.to_string())?;
        Ok(applied)
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("应用自动打标失败: {}", e);
        0
    })
}
