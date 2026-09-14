//! 上传 URL 拼接与阶段触发纯函数。

/// 确保 URL 带有协议前缀（默认 http://）
pub fn ensure_scheme(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    }
}

/// 拼接单次上传 URL（对齐 Python get_upload_url）
pub fn build_upload_url(base_url: &str) -> String {
    let mut url = ensure_scheme(base_url.trim_end_matches('/'));
    if !url.contains("/api/lol/upload") {
        url.push_str("/api/lol/upload");
    }
    url
}

/// 拼接批量上传 URL（对齐 Python get_batch_upload_url）
pub fn build_batch_upload_url(base_url: &str) -> String {
    let mut url = ensure_scheme(base_url.trim_end_matches('/'));
    if url.ends_with("/api/lol/upload") {
        url.truncate(url.len() - "/api/lol/upload".len());
    }
    url.push_str("/api/lol/upload-batch");
    url
}

/// 游戏结束上传触发条件：当前阶段为结算/空闲，且前一阶段为实际游戏运行状态。
/// 排除选人/确认阶段，避免秒退或拒绝匹配误触发上传。
pub fn should_trigger_upload_on_phase(phase: &str, prev: &str) -> bool {
    let is_end_phase = matches!(phase, "EndOfGame" | "Lobby" | "None");
    let was_in_game = matches!(
        prev,
        "InProgress" | "GameStart" | "PreEndOfGame" | "Reconnect"
    );
    is_end_phase && was_in_game
}
