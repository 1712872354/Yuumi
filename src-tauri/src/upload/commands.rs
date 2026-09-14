//! 上传相关 Tauri 命令。

use tauri::Manager;

use super::service::{batch_upload_by_ids, BatchUploadResult};
use super::url::build_batch_upload_url;

/// 单场上传 Tauri 命令（供 Career 自动上传 / Search 手动调用）
#[tauri::command]
pub async fn upload_single_match(
    game_id: u64,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let queue = {
        let state = app_handle.state::<crate::AppState>();
        state.upload_queue.clone()
    };
    queue.enqueue(game_id).await;
    Ok("已加入上传队列".to_string())
}

/// 批量上传 Tauri 命令（供 Search 页面自动/手动调用）
#[tauri::command]
pub async fn batch_upload_matches(
    game_ids: Vec<u64>,
    app_handle: tauri::AppHandle,
    app_state: tauri::State<'_, crate::AppState>,
) -> Result<BatchUploadResult, String> {
    if game_ids.is_empty() {
        return Ok(BatchUploadResult {
            success_count: 0,
            failed_count: 0,
            error: None,
        });
    }

    let raw_url = app_state.config.read().await.general.upload_api_url.clone();
    if raw_url.is_empty() {
        return Ok(BatchUploadResult {
            success_count: 0,
            failed_count: game_ids.len() as u32,
            error: Some("未配置上传 API 地址，请在设置 → 通用中配置".to_string()),
        });
    }
    let batch_url = build_batch_upload_url(&raw_url);
    log::info!(
        "[batch_upload] 开始批量上传 {} 场对局, url={}",
        game_ids.len(),
        batch_url
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        batch_upload_by_ids(&app_handle, &game_ids, &batch_url),
    )
    .await
    .unwrap_or(BatchUploadResult {
        success_count: 0,
        failed_count: game_ids.len() as u32,
        error: Some("批量上传超时 (120s)".to_string()),
    });

    log::info!(
        "[batch_upload] 完成: 成功={}, 失败={}",
        result.success_count,
        result.failed_count
    );
    Ok(result)
}
