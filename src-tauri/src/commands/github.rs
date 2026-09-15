use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::Manager;

/// GitHub 请求客户端（代理配置不变时进程级复用，变更时才重建）
struct GhClientEntry {
    enable_proxy: bool,
    proxy_addr: String,
    client: reqwest::Client,
}

static GITHUB_CLIENT: OnceLock<Mutex<Option<GhClientEntry>>> = OnceLock::new();

fn get_github_client(enable_proxy: bool, proxy_addr: &str) -> reqwest::Client {
    let cache = GITHUB_CLIENT.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = guard.as_ref() {
        if entry.enable_proxy == enable_proxy && entry.proxy_addr == proxy_addr {
            return entry.client.clone();
        }
    }

    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(15));
    if enable_proxy && !proxy_addr.is_empty() {
        let proxy_url = if proxy_addr.contains("://") {
            proxy_addr.to_string()
        } else {
            format!("http://{}", proxy_addr)
        };
        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                builder = builder.proxy(proxy);
                log::info!("[fetch_github_text] 已启用 GitHub 代理: {proxy_url}");
            }
            Err(e) => log::warn!("[fetch_github_text] 代理地址解析失败，将直连: {e}"),
        }
    }

    let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());
    *guard = Some(GhClientEntry {
        enable_proxy,
        proxy_addr: proxy_addr.to_string(),
        client: client.clone(),
    });
    client
}

/// 通过配置的 GitHub HTTP 代理请求远程文本内容（仅限 GitHub 域名，用于公告/版本历史拉取）
#[tauri::command]
pub async fn fetch_github_text(app: tauri::AppHandle, url: String) -> Result<String, String> {
    let parsed = url::Url::parse(&url).map_err(|e| format!("无效的 URL: {e}"))?;
    let host = parsed.host_str().unwrap_or("").to_string();
    if host != "github.com"
        && host != "api.github.com"
        && host != "raw.githubusercontent.com"
        && !host.ends_with(".githubusercontent.com")
    {
        return Err(format!("仅允许访问 GitHub 域名: {host}"));
    }

    let (enable_proxy, proxy_addr) = {
        let state = app.state::<AppState>();
        let cfg = state.config.read().await;
        (
            cfg.general.enable_http_proxy,
            cfg.general.http_proxy_addr.clone(),
        )
    };

    // 复用进程级缓存的客户端（代理配置不变时不重建，避免每次请求都新建）
    let client = get_github_client(enable_proxy, &proxy_addr);
    let resp = client
        .get(&url)
        .header("User-Agent", "Yuumi/1.0")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("读取响应失败: {e}"))
}

/// 版本更新日志缓存条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseEntry {
    pub tag: String,
    pub published_at: String,
    pub body: String,
}

/// 缓存文件: <data_dir>/releases_cache.json（便携版为 exe 旁 data/）
fn releases_cache_path() -> PathBuf {
    crate::runtime::app_data_dir().join("releases_cache.json")
}

/// 归一化版本号（去掉前缀 v 与首尾空白）
fn normalize_version(v: &str) -> String {
    v.trim().trim_start_matches('v').trim().to_string()
}

/// 获取 GitHub 版本更新日志（带本地缓存，缓存有效期 24 小时，
/// 避免频繁请求触发 GitHub API 未认证限流）。
/// 传入当前版本号时，若缓存中缺少该版本对应的 release
/// （例如刚通过更新器升到新版本），则视为缓存过期强制重新拉取。
#[tauri::command]
pub async fn get_release_changelog(
    app: tauri::AppHandle,
    current_version: Option<String>,
) -> Result<Vec<ReleaseEntry>, String> {
    const CACHE_TTL_SECS: u64 = 24 * 60 * 60;
    let cache_path = releases_cache_path();

    // 1. 尝试读缓存
    if cache_path.exists() {
        if let Ok(text) = std::fs::read_to_string(&cache_path) {
            if let Ok(cache) = serde_json::from_str::<Vec<ReleaseEntry>>(&text) {
                let is_stale = std::fs::metadata(&cache_path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d.as_secs() > CACHE_TTL_SECS)
                    .unwrap_or(true);
                // 缓存中是否缺少当前版本对应的 release（刚升级到新版本时缓存通常还没有它）
                let current = current_version
                    .as_deref()
                    .map(normalize_version)
                    .filter(|v| !v.is_empty());
                let missing_current = match &current {
                    Some(v) => !cache.iter().any(|e| normalize_version(&e.tag) == *v),
                    None => false,
                };
                if !is_stale && !missing_current && !cache.is_empty() {
                    log::info!("[get_release_changelog] 命中本地缓存，跳过 GitHub 请求");
                    return Ok(cache);
                }
            }
        }
    }

    // 2. 缓存未命中或过期，请求 GitHub
    log::info!("[get_release_changelog] 缓存未命中，请求 GitHub Releases API");
    let text = fetch_github_text(
        app.clone(),
        "https://api.github.com/repos/1712872354/Yuumi/releases".into(),
    )
    .await?;

    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("解析版本数据失败: {e}"))?;
    let arr = data
        .as_array()
        .ok_or_else(|| "版本数据格式错误".to_string())?;
    let releases: Vec<ReleaseEntry> = arr
        .iter()
        .map(|rel| ReleaseEntry {
            tag: rel
                .get("tag_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            published_at: rel
                .get("published_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            body: rel
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    // 3. 写缓存
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string(&releases) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&cache_path, json) {
                log::warn!("写入版本日志缓存失败: {e}");
            }
        }
        Err(e) => log::warn!("序列化版本日志缓存失败: {e}"),
    }

    Ok(releases)
}
