use rusqlite::params;

use crate::lcu::client::lcu_request;
use crate::AppState;

use super::types::{row_to_saved_player, SavedPlayerDto, SAVED_PLAYER_COLS};
use super::with_db;

// ─── 导出 / 导入 tag JSON ───

/// 回填历史保存玩家的 Riot ID（tagLine）：通过 LCU 按 puuid 查询召唤师信息补全 tag_line。
/// 返回更新的记录数。
#[tauri::command]
pub async fn backfill_saved_player_identity(
    app_state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let app_state = app_state.inner();

    let targets: Vec<(String, String)> = with_db(app_state, |conn| {
        let mut stmt = conn
            .prepare(
                "SELECT puuid, self_puuid FROM saved_players WHERE tag_line IS NULL OR tag_line = '' OR summoner_name IS NULL OR summoner_name = '' OR profile_icon_id = 0",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    })
    .await?;

    if targets.is_empty() {
        return Ok(0);
    }

    use futures_util::StreamExt;

    // 1. 并发获取 LCU 数据，限流并发数为 8
    let mut stream = futures_util::stream::iter(targets)
        .map(|(puuid, self_puuid)| async move {
            if puuid.is_empty() {
                return None;
            }
            let path = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
            match lcu_request(app_state, "GET", &path, None).await {
                Ok(info) => {
                    let tag_line = info
                        .get("tagLine")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let summoner_name = info
                        .get("displayName")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            info.get("gameName")
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.is_empty())
                        })
                        .unwrap_or("")
                        .to_string();
                    let profile_icon_id = info
                        .get("profileIconId")
                        .and_then(|v| v.as_i64())
                        .filter(|n| *n > 0)
                        .unwrap_or(0) as i32;

                    if tag_line.is_empty() && summoner_name.is_empty() && profile_icon_id == 0 {
                        None
                    } else {
                        Some((puuid, self_puuid, tag_line, summoner_name, profile_icon_id))
                    }
                }
                Err(_) => None,
            }
        })
        .buffer_unordered(8);

    let mut fetched_results = Vec::new();
    while let Some(res) = stream.next().await {
        if let Some(data) = res {
            fetched_results.push(data);
        }
    }

    // 2. 批量写入 SQLite (在一个事务中进行)
    let mut updated = 0u32;
    if !fetched_results.is_empty() {
        let db = app_state.saved_db.clone();
        updated = tauri::async_runtime::spawn_blocking(move || {
            let mut conn = db.lock().unwrap_or_else(|e| e.into_inner());
            let tx = conn.transaction().map_err(|e| e.to_string())?;

            {
                let mut stmt = tx
                    .prepare(
                        "UPDATE saved_players
                         SET tag_line = CASE WHEN tag_line IS NULL OR tag_line = '' THEN ?1 ELSE tag_line END,
                             summoner_name = CASE WHEN summoner_name IS NULL OR summoner_name = '' THEN ?2 ELSE summoner_name END,
                             profile_icon_id = CASE WHEN profile_icon_id = 0 THEN ?3 ELSE profile_icon_id END
                         WHERE puuid = ?4 AND self_puuid = ?5",
                    )
                    .map_err(|e| e.to_string())?;

                for (puuid, self_puuid, tag_line, summoner_name, profile_icon_id) in
                    fetched_results
                {
                    match stmt.execute(params![
                        tag_line,
                        summoner_name,
                        profile_icon_id,
                        puuid,
                        self_puuid
                    ]) {
                        Ok(_) => updated += 1,
                        Err(e) => log::warn!("回填召唤师信息失败: {}", e),
                    }
                }
            }

            tx.commit().map_err(|e| e.to_string())?;
            Ok::<u32, String>(updated)
        })
        .await
        .map_err(|e| format!("数据库任务异常终止: {}", e))??;
    }

    if updated > 0 {
        log::info!("已回填 {} 名保存玩家的身份信息", updated);
    }
    Ok(updated)
}

/// 导出所有带 tag 的玩家到用户选择的 JSON 文件。返回保存路径（取消则 None）。
#[tauri::command]
pub async fn export_tagged_players_to_json_file(
    app_state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    // 先短暂持锁读出数据，再在不持数据库锁的阻塞线程里弹文件对话框与写盘，
    // 避免模态框打开期间阻塞其他数据库操作
    let data: Vec<SavedPlayerDto> = with_db(app_state.inner(), |conn| {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM saved_players WHERE tag IS NOT NULL AND tag != ''",
                SAVED_PLAYER_COLS
            ))
            .map_err(|e| e.to_string())?;
        let data = stmt
            .query_map([], row_to_saved_player)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(data)
    })
    .await?;

    tauri::async_runtime::spawn_blocking(move || {
        let file = rfd::FileDialog::new()
            .set_title("导出标记玩家")
            .add_filter("JSON", &["json"])
            .set_file_name("tagged_players.json")
            .save_file();
        let Some(path) = file else {
            return Ok(None);
        };
        let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(Some(path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("导出任务异常终止: {}", e))?
}

/// 从用户选择的 JSON 文件导入标记玩家。返回导入数量（取消则 0）。
#[tauri::command]
pub async fn import_tagged_players_from_json_file(
    app_state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    // 先在不持数据库锁的阻塞线程里弹文件对话框并解析，再短暂持锁批量写入，
    // 避免模态框打开期间阻塞其他数据库操作
    let records: Vec<SavedPlayerDto> = tauri::async_runtime::spawn_blocking(|| {
        let file = rfd::FileDialog::new()
            .set_title("导入标记玩家")
            .add_filter("JSON", &["json"])
            .pick_file();
        let Some(path) = file else {
            return Ok(Vec::new());
        };
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("导入任务异常终止: {}", e))??;

    if records.is_empty() {
        return Ok(0);
    }

    with_db(app_state.inner(), move |conn| {
        let mut count = 0u32;
        for r in records {
            let result = conn.execute(
                "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
                   tag = excluded.tag,
                   summoner_name = excluded.summoner_name,
                   profile_icon_id = excluded.profile_icon_id,
                   tag_line = excluded.tag_line,
                   champion_id = excluded.champion_id,
                   update_at = excluded.update_at,
                   last_met_at = excluded.last_met_at",
                params![
                    r.puuid,
                    r.self_puuid,
                    r.region,
                    r.rso_platform_id,
                    r.tag,
                    r.summoner_name,
                    r.profile_icon_id,
                    r.tag_line,
                    r.champion_id,
                    r.update_at,
                    r.last_met_at,
                ],
            );
            if let Err(e) = result {
                log::warn!("导入玩家失败: {}", e);
                continue;
            }
            count += 1;
        }
        Ok(count)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE saved_players (
                puuid TEXT NOT NULL,
                self_puuid TEXT NOT NULL,
                region TEXT NOT NULL DEFAULT '',
                rso_platform_id TEXT NOT NULL DEFAULT '',
                tag TEXT,
                summoner_name TEXT NOT NULL DEFAULT '',
                profile_icon_id INTEGER NOT NULL DEFAULT 0,
                update_at INTEGER NOT NULL,
                last_met_at INTEGER,
                tag_line TEXT,
                champion_id INTEGER NOT NULL DEFAULT 0,
                auto_tag TEXT,
                auto_score REAL,
                auto_reason TEXT,
                auto_tag_stats TEXT,
                last_relation TEXT,
                PRIMARY KEY (puuid, self_puuid, region, rso_platform_id)
             );
             CREATE TABLE encountered_games (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id INTEGER NOT NULL,
                puuid TEXT NOT NULL,
                self_puuid TEXT NOT NULL,
                region TEXT NOT NULL DEFAULT '',
                rso_platform_id TEXT NOT NULL DEFAULT '',
                queue_type TEXT NOT NULL DEFAULT '',
                update_at INTEGER NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    /// 与 record_encounters 相同的 UPDATE 语义：仅更新已存在行
    fn update_existing_only(
        conn: &Connection,
        puuid: &str,
        self_puuid: &str,
        name: &str,
        relation: &str,
        ts: i64,
    ) -> usize {
        conn.execute(
            "UPDATE saved_players SET
               summoner_name = CASE WHEN ?1 = '' THEN summoner_name ELSE ?1 END,
               last_relation = CASE WHEN ?2 = '' THEN last_relation ELSE ?2 END,
               update_at = ?3,
               last_met_at = ?3
             WHERE puuid = ?4 AND self_puuid = ?5",
            params![name, relation, ts, puuid, self_puuid],
        )
        .unwrap()
    }

    #[test]
    fn record_encounters_skips_unknown_players() {
        let conn = mem_db();
        let changed = update_existing_only(&conn, "newbie", "me", "Foo", "ally", 1);
        assert_eq!(changed, 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM saved_players", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn record_encounters_updates_existing_and_relation() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, update_at)
             VALUES ('p1', 'me', '', '', '老坑', 'OldName', 0)",
            [],
        )
        .unwrap();

        let changed = update_existing_only(&conn, "p1", "me", "NewName", "enemy", 999);
        assert_eq!(changed, 1);

        let (name, tag, rel, met): (String, Option<String>, Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT summoner_name, tag, last_relation, last_met_at FROM saved_players WHERE puuid='p1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(name, "NewName");
        assert_eq!(tag.as_deref(), Some("老坑"), "手动 tag 不应被相遇更新覆盖");
        assert_eq!(rel.as_deref(), Some("enemy"));
        assert_eq!(met, Some(999));
    }

    #[test]
    fn auto_tag_upsert_inserts_new_player() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, update_at, last_met_at, champion_id, last_relation)
             VALUES ('carry', 'me', '', '', NULL, 'CarryGuy', 0, 1, 1, 99, 'ally')
             ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
               summoner_name = excluded.summoner_name,
               update_at = excluded.update_at,
               last_met_at = excluded.last_met_at",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE saved_players SET auto_tag='大腿', auto_score=88, auto_reason='胜 · KDA 12.0' WHERE puuid='carry'",
            [],
        )
        .unwrap();

        let auto_tag: Option<String> = conn
            .query_row(
                "SELECT auto_tag FROM saved_players WHERE puuid='carry'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(auto_tag.as_deref(), Some("大腿"));
    }
}
