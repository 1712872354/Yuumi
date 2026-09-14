use crate::lcu::client::lcu_request;
use crate::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SQLite 数据库文件路径：<data_dir>/saved_players.db
fn db_path() -> std::path::PathBuf {
    crate::runtime::app_data_dir().join("saved_players.db")
}

/// 打开（必要时创建）数据库并建表
pub fn init_db() -> rusqlite::Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS saved_players (
            puuid TEXT NOT NULL,
            self_puuid TEXT NOT NULL,
            region TEXT NOT NULL DEFAULT '',
            rso_platform_id TEXT NOT NULL DEFAULT '',
            tag TEXT,
            summoner_name TEXT NOT NULL DEFAULT '',
            profile_icon_id INTEGER NOT NULL DEFAULT 0,
            update_at INTEGER NOT NULL,
            last_met_at INTEGER,
            PRIMARY KEY (puuid, self_puuid, region, rso_platform_id)
         );
         CREATE TABLE IF NOT EXISTS encountered_games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id INTEGER NOT NULL,
            puuid TEXT NOT NULL,
            self_puuid TEXT NOT NULL,
            region TEXT NOT NULL DEFAULT '',
            rso_platform_id TEXT NOT NULL DEFAULT '',
            queue_type TEXT NOT NULL DEFAULT '',
            update_at INTEGER NOT NULL
         );
          CREATE INDEX IF NOT EXISTS idx_encountered_puuid
              ON encountered_games (puuid, self_puuid, queue_type);
          CREATE TABLE IF NOT EXISTS pending_uploads (
             game_id INTEGER PRIMARY KEY,
             payload TEXT NOT NULL,
             retry_count INTEGER NOT NULL DEFAULT 1,
             last_error TEXT NOT NULL DEFAULT '',
             created_at TEXT NOT NULL
          );",
    )?;

    // 迁移：旧数据库补充 tag_line / champion_id 列
    let existing_cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(saved_players)")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if !existing_cols.iter().any(|c| c == "tag_line") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN tag_line TEXT;")?;
    }
    if !existing_cols.iter().any(|c| c == "champion_id") {
        conn.execute_batch(
            "ALTER TABLE saved_players ADD COLUMN champion_id INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    // 自动打标字段
    if !existing_cols.iter().any(|c| c == "auto_tag") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN auto_tag TEXT;")?;
    }
    if !existing_cols.iter().any(|c| c == "auto_score") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN auto_score REAL;")?;
    }
    if !existing_cols.iter().any(|c| c == "auto_reason") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN auto_reason TEXT;")?;
    }
    if !existing_cols.iter().any(|c| c == "auto_tag_stats") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN auto_tag_stats TEXT;")?;
    }
    // 最近一次关系：ally / enemy
    if !existing_cols.iter().any(|c| c == "last_relation") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN last_relation TEXT;")?;
    }
    // 黑白名单：list_kind = '' | 'black' | 'white'；list_reason 为拉黑/加白理由
    if !existing_cols.iter().any(|c| c == "list_kind") {
        conn.execute_batch(
            "ALTER TABLE saved_players ADD COLUMN list_kind TEXT NOT NULL DEFAULT '';",
        )?;
    }
    if !existing_cols.iter().any(|c| c == "list_reason") {
        conn.execute_batch("ALTER TABLE saved_players ADD COLUMN list_reason TEXT;")?;
    }

    // 迁移：将因此前数据采集 bug 误记录为 '0' 的对局模式默认全部更新为海克斯大乱斗 '2400'
    let _ = conn.execute(
        "UPDATE encountered_games SET queue_type = '2400' WHERE queue_type = '0';",
        [],
    );

    Ok(conn)
}

/// 在阻塞线程池执行数据库闭包，避免 SQLite 同步 I/O 占用 tokio 工作线程。
/// （async 命令的统一入口：克隆 Arc 进闭包，锁在阻塞线程内获取释放）
async fn with_db<T, F>(state: &AppState, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T, String> + Send + 'static,
{
    let db = state.saved_db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut conn = db.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut conn)
    })
    .await
    .map_err(|e| format!("数据库任务异常终止: {}", e))?
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ─── 数据结构 ───

/// 本局缓存：对局结束记录相遇时使用
#[derive(Debug, Clone)]
pub struct CurrentGameCache {
    pub game_id: i64,
    pub queue_id: i32,
    pub players: Vec<GamePlayerEntry>,
}

/// 单个玩家的对局信息
#[derive(Debug, Clone)]
pub struct GamePlayerEntry {
    pub puuid: String,
    pub summoner_name: String,
    pub profile_icon_id: i32,
    pub tag_line: Option<String>,
    pub champion_id: i32,
    /// "ally" / "enemy" / ""（未知）
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlayerDto {
    pub puuid: String,
    pub self_puuid: String,
    pub region: String,
    pub rso_platform_id: String,
    pub tag: Option<String>,
    pub summoner_name: String,
    pub profile_icon_id: i32,
    #[serde(default)]
    pub tag_line: Option<String>,
    #[serde(default)]
    pub champion_id: i32,
    pub update_at: i64,
    pub last_met_at: Option<i64>,
    #[serde(default)]
    pub last_queue_type: Option<String>,
    #[serde(default)]
    pub encounter_count: i32,
    /// 最近自动标签（大腿/坑/演员等）
    #[serde(default)]
    pub auto_tag: Option<String>,
    #[serde(default)]
    pub auto_score: Option<f64>,
    #[serde(default)]
    pub auto_reason: Option<String>,
    #[serde(default)]
    pub auto_tag_stats: Option<String>,
    /// 最近一次同局关系：ally / enemy
    #[serde(default)]
    pub last_relation: Option<String>,
    /// 名单类型：'' 普通 / 'black' 拉黑 / 'white' 加白
    #[serde(default)]
    pub list_kind: String,
    #[serde(default)]
    pub list_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounteredGameDto {
    pub id: i64,
    pub game_id: i64,
    pub puuid: String,
    pub self_puuid: String,
    pub region: String,
    pub rso_platform_id: String,
    pub queue_type: String,
    pub update_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult<T> {
    pub data: Vec<T>,
    pub count: i64,
}

fn row_to_saved_player(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedPlayerDto> {
    Ok(SavedPlayerDto {
        puuid: row.get(0)?,
        self_puuid: row.get(1)?,
        region: row.get(2)?,
        rso_platform_id: row.get(3)?,
        tag: row.get(4)?,
        summoner_name: row.get(5)?,
        profile_icon_id: row.get(6)?,
        tag_line: row.get(7)?,
        champion_id: row.get(8)?,
        update_at: row.get(9)?,
        last_met_at: row.get(10)?,
        auto_tag: row.get(11).ok().flatten(),
        auto_score: row.get(12).ok().flatten(),
        auto_reason: row.get(13).ok().flatten(),
        auto_tag_stats: row.get(14).ok().flatten(),
        last_relation: row.get(15).ok().flatten(),
        list_kind: row.get::<_, String>(16).unwrap_or_default(),
        list_reason: row.get(17).ok().flatten(),
        last_queue_type: row.get(18).ok().flatten(),
        encounter_count: row.get::<usize, i32>(19).unwrap_or(1),
    })
}

const SAVED_PLAYER_COLS: &str =
    "puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at, auto_tag, auto_score, auto_reason, auto_tag_stats, last_relation, list_kind, list_reason";

fn row_to_encountered(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncounteredGameDto> {
    Ok(EncounteredGameDto {
        id: row.get(0)?,
        game_id: row.get(1)?,
        puuid: row.get(2)?,
        self_puuid: row.get(3)?,
        region: row.get(4)?,
        rso_platform_id: row.get(5)?,
        queue_type: row.get(6)?,
        update_at: row.get(7)?,
    })
}

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
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let key = t.tag.as_key().to_string();
            *stats.entry(key).or_insert(0) += 1;
            let stats_str = serde_json::to_string(&stats).unwrap_or_else(|_| "{}".into());

            let _ = tx.execute(
                "UPDATE saved_players SET auto_tag = ?1, auto_score = ?2, auto_reason = ?3, auto_tag_stats = ?4, update_at = ?5
                 WHERE puuid = ?6 AND self_puuid = ?7",
                params![
                    t.tag.as_str(),
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

// ─── Tauri 命令 ───

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSavedPlayerInput {
    pub puuid: String,
    pub self_puuid: String,
    #[serde(default)]
    pub rso_platform_id: String,
    #[serde(default)]
    pub region: String,
    pub tag: Option<String>,
    #[serde(default)]
    pub summoner_name: String,
    #[serde(default)]
    pub profile_icon_id: i32,
    #[serde(default)]
    pub encountered: bool,
    #[serde(default)]
    pub champion_id: i32,
}

/// 保存/更新玩家记录（upsert）。tag 为 None 时保留已有 tag，Some 时覆盖。
#[tauri::command]
pub async fn save_saved_player(
    app_state: tauri::State<'_, AppState>,
    dto: SaveSavedPlayerInput,
) -> Result<(), String> {
    with_db(app_state.inner(), move |conn| {
        let ts = now();
        let last_met = if dto.encountered { Some(ts) } else { None };
        conn.execute(
            "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?10, ?8, ?9)
             ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
               tag = CASE WHEN excluded.tag IS NULL THEN saved_players.tag ELSE excluded.tag END,
               summoner_name = CASE WHEN excluded.summoner_name = '' THEN saved_players.summoner_name ELSE excluded.summoner_name END,
               profile_icon_id = CASE WHEN excluded.profile_icon_id = 0 THEN saved_players.profile_icon_id ELSE excluded.profile_icon_id END,
               champion_id = CASE WHEN excluded.champion_id = 0 THEN saved_players.champion_id ELSE excluded.champion_id END,
               update_at = excluded.update_at,
               last_met_at = COALESCE(excluded.last_met_at, saved_players.last_met_at)",
            params![
                dto.puuid,
                dto.self_puuid,
                dto.region,
                dto.rso_platform_id,
                dto.tag,
                dto.summoner_name,
                dto.profile_icon_id,
                ts,
                last_met,
                dto.champion_id,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// 设置黑白名单：kind = '' 清除 / 'black' 拉黑 / 'white' 加白
/// 玩家不存在时自动 upsert 一行（便于直接从战绩/雷达拉黑）
#[tauri::command]
pub async fn set_player_list_kind(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
    puuid: String,
    kind: String,
    reason: Option<String>,
    summoner_name: Option<String>,
) -> Result<(), String> {
    let kind = match kind.as_str() {
        "black" | "white" | "" => kind,
        other => return Err(format!("无效名单类型: {other}")),
    };
    with_db(app_state.inner(), move |conn| {
        let ts = now();
        conn.execute(
            "INSERT INTO saved_players (puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, update_at, last_met_at, list_kind, list_reason)
             VALUES (?1, ?2, '', '', NULL, ?3, 0, ?4, ?4, ?5, ?6)
             ON CONFLICT(puuid, self_puuid, region, rso_platform_id) DO UPDATE SET
               list_kind = excluded.list_kind,
               list_reason = excluded.list_reason,
               summoner_name = CASE WHEN excluded.summoner_name = '' THEN saved_players.summoner_name ELSE excluded.summoner_name END,
               update_at = excluded.update_at",
            params![
                puuid,
                self_puuid,
                summoner_name.unwrap_or_default(),
                ts,
                kind,
                reason
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// filter: "tagged" 只看已标记玩家，"multiple" 只看多次相遇玩家，其他为全部
#[tauri::command]
pub async fn query_all_saved_players(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
    page: Option<i64>,
    page_size: Option<i64>,
    filter: Option<String>,
) -> Result<PageResult<SavedPlayerDto>, String> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(50).clamp(1, 200);
    let where_clause = match filter.as_deref() {
        Some("tagged") => {
            " AND ((tag IS NOT NULL AND tag != '') OR (auto_tag IS NOT NULL AND auto_tag != '') OR list_kind IN ('black', 'white'))"
        }
        Some("black") => " AND list_kind = 'black'",
        Some("white") => " AND list_kind = 'white'",
        Some("multiple") => " AND (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = saved_players.puuid AND eg.self_puuid = saved_players.self_puuid) >= 2",
        _ => "",
    }
    .to_string();
    with_db(app_state.inner(), move |conn| {
        let count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM saved_players WHERE self_puuid = ?1{where_clause}"),
                params![self_puuid],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {}, \
                 (SELECT queue_type FROM encountered_games eg WHERE eg.puuid = saved_players.puuid AND eg.self_puuid = saved_players.self_puuid ORDER BY eg.update_at DESC LIMIT 1), \
                 (SELECT COUNT(*) FROM encountered_games eg WHERE eg.puuid = saved_players.puuid AND eg.self_puuid = saved_players.self_puuid) AS encounter_cnt \
                 FROM saved_players WHERE self_puuid = ?1{where_clause} \
                 ORDER BY last_met_at DESC, update_at DESC LIMIT ?2 OFFSET ?3",
                SAVED_PLAYER_COLS
            ))
            .map_err(|e| e.to_string())?;
        let data = stmt
            .query_map(
                params![self_puuid, page_size, (page - 1) * page_size],
                row_to_saved_player,
            )
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(PageResult { data, count })
    })
    .await
}

/// 保存玩家的精简标记（对局信息页徽章 / 选人雷达用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlayerMarker {
    pub tag: Option<String>,
    pub encounter_count: i32,
    /// 最近自动标签（大腿/坑等）
    #[serde(default)]
    pub auto_tag: Option<String>,
    /// 最近一次同局关系
    #[serde(default)]
    pub last_relation: Option<String>,
    /// 最近相遇时间戳 ms
    #[serde(default)]
    pub last_met_at: Option<i64>,
    /// '' / black / white
    #[serde(default)]
    pub list_kind: String,
    #[serde(default)]
    pub list_reason: Option<String>,
}

/// 获取全部保存玩家的精简映射：puuid → 标记信息（tag + 相遇次数）
#[tauri::command]
pub async fn get_saved_players_map(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
) -> Result<HashMap<String, SavedPlayerMarker>, String> {
    Ok(query_saved_players_map(app_state.inner(), self_puuid).await)
}

pub async fn query_saved_players_map(
    app_state: &AppState,
    self_puuid: String,
) -> HashMap<String, SavedPlayerMarker> {
    if self_puuid.is_empty() {
        return HashMap::new();
    }
    with_db(app_state, move |conn| {
        let mut stmt = conn
            .prepare(
                "SELECT sp.puuid,
                        sp.tag,
                        (SELECT COUNT(*) FROM encountered_games eg
                         WHERE eg.puuid = sp.puuid AND eg.self_puuid = sp.self_puuid),
                        sp.auto_tag,
                        sp.last_relation,
                        sp.last_met_at,
                        sp.list_kind,
                        sp.list_reason
                 FROM saved_players sp
                 WHERE sp.self_puuid = ?1
                   AND (
                     (sp.tag IS NOT NULL AND sp.tag != '')
                     OR (sp.auto_tag IS NOT NULL AND sp.auto_tag != '')
                     OR sp.list_kind IN ('black', 'white')
                   )",
            )
            .map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        let rows = stmt
            .query_map(params![self_puuid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    SavedPlayerMarker {
                        tag: r.get(1)?,
                        encounter_count: r.get::<_, i32>(2).unwrap_or(1),
                        auto_tag: r.get(3).ok().flatten(),
                        last_relation: r.get(4).ok().flatten(),
                        last_met_at: r.get(5).ok().flatten(),
                        list_kind: r.get::<_, String>(6).unwrap_or_default(),
                        list_reason: r.get(7).ok().flatten(),
                    },
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (puuid, marker) = row.map_err(|e| e.to_string())?;
            map.insert(puuid, marker);
        }
        Ok(map)
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("查询已标记玩家映射失败: {}", e);
        HashMap::new()
    })
}

/// 分页查询与某玩家的相遇对局记录（按时间倒序）
#[tauri::command]
pub async fn query_encountered_games(
    app_state: tauri::State<'_, AppState>,
    self_puuid: String,
    puuid: String,
    queue_type: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PageResult<EncounteredGameDto>, String> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    let q = queue_type.unwrap_or_default();

    with_db(app_state.inner(), move |conn| {
        let count: i64 = if q.is_empty() {
            conn.query_row(
                "SELECT COUNT(*) FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2",
                params![self_puuid, puuid],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2 AND queue_type = ?3",
                params![self_puuid, puuid, q],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?
        };

        let data = if q.is_empty() {
            let mut stmt = conn
                .prepare(
                    "SELECT id, game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2 ORDER BY update_at DESC LIMIT ?3 OFFSET ?4",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    params![self_puuid, puuid, page_size, (page - 1) * page_size],
                    row_to_encountered,
                )
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT id, game_id, puuid, self_puuid, region, rso_platform_id, queue_type, update_at FROM encountered_games WHERE self_puuid = ?1 AND puuid = ?2 AND queue_type = ?3 ORDER BY update_at DESC LIMIT ?4 OFFSET ?5",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(
                    params![self_puuid, puuid, q, page_size, (page - 1) * page_size],
                    row_to_encountered,
                )
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        Ok(PageResult { data, count })
    })
    .await
}

/// 删除保存玩家及其相遇记录
#[tauri::command]
pub async fn delete_saved_player(
    app_state: tauri::State<'_, AppState>,
    puuid: String,
    self_puuid: String,
) -> Result<(), String> {
    with_db(app_state.inner(), move |conn| {
        conn.execute(
            "DELETE FROM saved_players WHERE puuid = ?1 AND self_puuid = ?2",
            params![puuid, self_puuid],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM encountered_games WHERE puuid = ?1 AND self_puuid = ?2",
            params![puuid, self_puuid],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

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
