//! 管道可观测性：轻量原子计数，供排障命令读取。
//! 不做指标上报，只保留进程内累计值。

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct PipelineStats {
    /// champ-select session 因 300ms 节流丢弃的事件数
    pub champ_select_throttled: AtomicU64,
    /// BP agent mpsc 通道已满丢弃的 session 帧
    pub bp_channel_dropped: AtomicU64,
    /// match-agent mpsc 通道已满丢弃的 gameflow 帧
    pub match_channel_dropped: AtomicU64,
    /// 前端已 emit 的 lcu-ws-event 数
    pub ws_events_emitted: AtomicU64,
    /// 对局详情缓存命中次数
    pub match_detail_cache_hits: AtomicU64,
    /// 对局详情缓存 miss（真正 HTTP）次数
    pub match_detail_cache_misses: AtomicU64,
    /// 上传入队次数
    pub upload_enqueued: AtomicU64,
    /// 上传成功次数
    pub upload_success: AtomicU64,
    /// 上传失败次数
    pub upload_failed: AtomicU64,
}

impl PipelineStats {
    pub fn snapshot(&self) -> PipelineStatsSnapshot {
        PipelineStatsSnapshot {
            champ_select_throttled: self.champ_select_throttled.load(Ordering::Relaxed),
            bp_channel_dropped: self.bp_channel_dropped.load(Ordering::Relaxed),
            match_channel_dropped: self.match_channel_dropped.load(Ordering::Relaxed),
            ws_events_emitted: self.ws_events_emitted.load(Ordering::Relaxed),
            match_detail_cache_hits: self.match_detail_cache_hits.load(Ordering::Relaxed),
            match_detail_cache_misses: self.match_detail_cache_misses.load(Ordering::Relaxed),
            upload_enqueued: self.upload_enqueued.load(Ordering::Relaxed),
            upload_success: self.upload_success.load(Ordering::Relaxed),
            upload_failed: self.upload_failed.load(Ordering::Relaxed),
        }
    }
}

/// 对外快照（camelCase，便于前端直接用）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStatsSnapshot {
    pub champ_select_throttled: u64,
    pub bp_channel_dropped: u64,
    pub match_channel_dropped: u64,
    pub ws_events_emitted: u64,
    pub match_detail_cache_hits: u64,
    pub match_detail_cache_misses: u64,
    pub upload_enqueued: u64,
    pub upload_success: u64,
    pub upload_failed: u64,
}

static STATS: PipelineStats = PipelineStats {
    champ_select_throttled: AtomicU64::new(0),
    bp_channel_dropped: AtomicU64::new(0),
    match_channel_dropped: AtomicU64::new(0),
    ws_events_emitted: AtomicU64::new(0),
    match_detail_cache_hits: AtomicU64::new(0),
    match_detail_cache_misses: AtomicU64::new(0),
    upload_enqueued: AtomicU64::new(0),
    upload_success: AtomicU64::new(0),
    upload_failed: AtomicU64::new(0),
};

pub fn stats() -> &'static PipelineStats {
    &STATS
}

pub fn incr_champ_select_throttled() {
    STATS.champ_select_throttled.fetch_add(1, Ordering::Relaxed);
}

pub fn incr_bp_channel_dropped() {
    STATS.bp_channel_dropped.fetch_add(1, Ordering::Relaxed);
}

pub fn incr_match_channel_dropped() {
    STATS.match_channel_dropped.fetch_add(1, Ordering::Relaxed);
}

pub fn incr_ws_events_emitted() {
    STATS.ws_events_emitted.fetch_add(1, Ordering::Relaxed);
}

pub fn incr_match_detail_cache_hit() {
    STATS
        .match_detail_cache_hits
        .fetch_add(1, Ordering::Relaxed);
}

pub fn incr_match_detail_cache_miss() {
    STATS
        .match_detail_cache_misses
        .fetch_add(1, Ordering::Relaxed);
}

pub fn incr_upload_enqueued() {
    STATS.upload_enqueued.fetch_add(1, Ordering::Relaxed);
}

pub fn incr_upload_success() {
    STATS.upload_success.fetch_add(1, Ordering::Relaxed);
}

pub fn incr_upload_failed() {
    STATS.upload_failed.fetch_add(1, Ordering::Relaxed);
}

/// 读取当前快照（调试 / Tools 页）
#[tauri::command]
pub fn get_pipeline_stats() -> PipelineStatsSnapshot {
    STATS.snapshot()
}
