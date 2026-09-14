//! 本地去重异步上传队列。

use std::collections::HashSet;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex};

use super::service::upload_single_task;
use super::store::{load_pending_uploads, UploadTask};
use super::url::build_upload_url;

/// 单条暂存上传的最大自动尝试次数。
/// 超过后（如服务器永久拒绝的 4xx 死信）不再自动重试，避免每局结束/启动时反复空传。
const MAX_UPLOAD_RETRY_COUNT: u32 = 10;

/// 本地去重异步上传队列状态机。
/// 内部维护 `mpsc::channel` + `HashSet<u64>` 去重 + 后台 Worker。
#[derive(Clone)]
pub struct UploadQueue {
    /// 上传请求发送端
    tx: mpsc::Sender<UploadTask>,
    /// 已入队的 gameId 集合（去重用）
    enqueued: Arc<Mutex<HashSet<u64>>>,
    app_handle: AppHandle,
}

impl UploadQueue {
    pub fn new(app_handle: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<UploadTask>(128);
        let enqueued = Arc::new(Mutex::new(HashSet::new()));

        let queue = Self {
            tx: tx.clone(),
            enqueued: enqueued.clone(),
            app_handle: app_handle.clone(),
        };

        // 启动后台 Worker（使用 Tauri 异步运行时）
        crate::spawn_log_panic(upload_worker(app_handle.clone(), rx, enqueued.clone()));

        // 启动应用恢复（仅启动时拉起 1 次，不在后台周期性循环）
        let queue_clone = queue.clone();
        crate::spawn_log_panic(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            queue_clone.trigger_pending_retry().await;
        });

        queue
    }

    /// 将 gameId 推入上传队列（自动去重）。
    /// 如果该 gameId 已在队列或已上传，直接跳过。
    pub async fn enqueue(&self, game_id: u64) {
        if game_id == 0 {
            return;
        }

        let mut set = self.enqueued.lock().await;
        if set.contains(&game_id) {
            log::debug!("对局 {} 已在上传队列中，跳过", game_id);
            return;
        }
        if let Err(e) = self.tx.try_send(UploadTask::GameId(game_id)) {
            log::warn!("推入上传队列失败: {}", e);
        } else {
            set.insert(game_id);
            crate::pipeline_stats::incr_upload_enqueued();
            log::info!("对局 {} 已加入上传队列", game_id);
        }
    }

    /// 读取本地 SQLite 数据库，将挂起的未完成任务全部拉起并排队重试
    pub async fn trigger_pending_retry(&self) {
        let pending_items = load_pending_uploads(&self.app_handle).await;
        if pending_items.is_empty() {
            return;
        }
        log::info!(
            "检测到 {} 条挂起未完成的对局上传记录，准备重新入队上传...",
            pending_items.len()
        );

        let mut set = self.enqueued.lock().await;
        for item in pending_items {
            let game_id = item.game_id;
            if item.retry_count >= MAX_UPLOAD_RETRY_COUNT {
                log::debug!(
                    "对局 {} 上传已尝试 {} 次，超过上限，跳过自动重试",
                    game_id,
                    item.retry_count
                );
                continue;
            }
            if !set.contains(&game_id) {
                if let Err(e) = self.tx.try_send(UploadTask::Pending(Box::new(item))) {
                    log::warn!("对局 {} 重新入队失败: {}", game_id, e);
                } else {
                    set.insert(game_id);
                }
            }
        }
    }
}

/// 串行处理上传队列，每局最多 35 秒超时。
async fn upload_worker(
    app_handle: AppHandle,
    mut rx: mpsc::Receiver<UploadTask>,
    enqueued: Arc<Mutex<HashSet<u64>>>,
) {
    log::info!("上传 Worker 已启动");

    while let Some(task) = rx.recv().await {
        let game_id = task.game_id();
        log::info!("开始上传对局: {}", game_id);

        // 从配置读取上传 URL（每次重新读取，支持运行时修改）
        let upload_url = {
            let state = app_handle.state::<crate::AppState>();
            let cfg = state.config.read().await;
            let raw = cfg.general.upload_api_url.clone();
            if raw.is_empty() {
                log::warn!("对局 {} 跳过上传: 未配置上传 API 地址", game_id);
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
                continue;
            }
            build_upload_url(&raw)
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(35),
            upload_single_task(&app_handle, task, &upload_url),
        )
        .await;

        match result {
            Ok(Ok(status)) => {
                log::info!("对局 {} 上传完成: {}", game_id, status);
                crate::pipeline_stats::incr_upload_success();
                if status == "new" {
                    let _ =
                        app_handle.emit("upload-success", serde_json::json!({ "gameId": game_id }));
                }
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
            }
            Ok(Err(e)) => {
                log::warn!("对局 {} 上传处理失败: {}", game_id, e);
                crate::pipeline_stats::incr_upload_failed();
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
            }
            Err(_) => {
                log::warn!("对局 {} 上传超时 (35s)", game_id);
                crate::pipeline_stats::incr_upload_failed();
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
            }
        }
    }

    log::info!("上传 Worker 已退出");
}
