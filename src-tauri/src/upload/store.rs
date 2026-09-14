//! SQLite 暂存上传记录（pending_uploads 表）。

use tauri::{AppHandle, Manager};

use super::payload::UploadPayload;

/// 暂存战绩条目，存储在 SQLite 数据库 pending_uploads 表中
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingUploadItem {
    pub game_id: u64,
    pub payload: UploadPayload,
    pub retry_count: u32,
    pub last_error: String,
    pub created_at: String,
}

/// 上传任务枚举：可以是待构建 Payload 的 game_id，也可以是已生成的 PendingUploadItem
#[derive(Debug, Clone)]
pub enum UploadTask {
    GameId(u64),
    Pending(Box<PendingUploadItem>),
}

impl UploadTask {
    pub fn game_id(&self) -> u64 {
        match self {
            UploadTask::GameId(id) => *id,
            UploadTask::Pending(item) => item.game_id,
        }
    }
}

/// 从 SQLite 数据库读取所有挂起的上传任务
pub fn load_pending_uploads(app_handle: &AppHandle) -> Vec<PendingUploadItem> {
    let state = app_handle.state::<crate::AppState>();
    let conn = state.saved_db.lock().unwrap_or_else(|e| e.into_inner());

    let mut stmt = match conn.prepare(
        "SELECT game_id, payload, retry_count, last_error, created_at FROM pending_uploads ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Prepare SELECT pending_uploads 失败: {}", e);
            return Vec::new();
        }
    };

    let rows = stmt.query_map([], |row| {
        let game_id: i64 = row.get(0)?;
        let payload_str: String = row.get(1)?;
        let retry_count: u32 = row.get(2)?;
        let last_error: String = row.get(3)?;
        let created_at: String = row.get(4)?;
        Ok((game_id, payload_str, retry_count, last_error, created_at))
    });

    let mut items = Vec::new();
    if let Ok(rows) = rows {
        for r in rows.flatten() {
            if let Ok(payload) = serde_json::from_str::<UploadPayload>(&r.1) {
                items.push(PendingUploadItem {
                    game_id: r.0 as u64,
                    payload,
                    retry_count: r.2,
                    last_error: r.3,
                    created_at: r.4,
                });
            }
        }
    }
    items
}

/// 添加或更新一条挂起上传记录到 SQLite 数据库
pub fn add_or_update_pending_upload(app_handle: &AppHandle, payload: UploadPayload, error: String) {
    let game_id = payload.match_info.match_id;
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let payload_json = match serde_json::to_string(&payload) {
        Ok(j) => j,
        Err(e) => {
            log::error!("序列化 UploadPayload 失败: {}", e);
            return;
        }
    };

    let state = app_handle.state::<crate::AppState>();
    let conn = state.saved_db.lock().unwrap_or_else(|e| e.into_inner());

    let existing_retry: Option<u32> = conn
        .query_row(
            "SELECT retry_count FROM pending_uploads WHERE game_id = ?1",
            rusqlite::params![game_id as i64],
            |r| r.get(0),
        )
        .ok();

    if let Some(count) = existing_retry {
        let _ = conn.execute(
            "UPDATE pending_uploads SET retry_count = ?1, last_error = ?2, payload = ?3 WHERE game_id = ?4",
            rusqlite::params![count + 1, error, payload_json, game_id as i64],
        );
    } else {
        let _ = conn.execute(
            "INSERT INTO pending_uploads (game_id, payload, retry_count, last_error, created_at) VALUES (?1, ?2, 1, ?3, ?4)",
            rusqlite::params![game_id as i64, payload_json, error, now],
        );
    }

    // 保留最新的 100 条，超出部分清理掉最旧的记录
    let _ = conn.execute(
        "DELETE FROM pending_uploads WHERE game_id NOT IN (
            SELECT game_id FROM pending_uploads ORDER BY created_at DESC LIMIT 100
        )",
        [],
    );
}

/// 上传成功后，清除该条 SQLite 数据库记录
pub fn remove_pending_upload(app_handle: &AppHandle, game_id: u64) {
    let state = app_handle.state::<crate::AppState>();
    let conn = state.saved_db.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = conn.execute(
        "DELETE FROM pending_uploads WHERE game_id = ?1",
        rusqlite::params![game_id as i64],
    ) {
        log::error!("删除 pending_uploads 记录 {} 失败: {}", game_id, e);
    }
}
