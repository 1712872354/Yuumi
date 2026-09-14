use rusqlite::Connection;

use crate::AppState;

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
pub(crate) async fn with_db<T, F>(state: &AppState, f: F) -> Result<T, String>
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

pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
