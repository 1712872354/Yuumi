// ─── 对局详情共享缓存 ───
// 上传 / 宿命 / 最近队友 / 自动打标 同一 gameId 短 TTL 内复用一次 LCU 拉取结果。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

/// 结算后详情在 LCU 内短时间不变；45s 覆盖对局结束扇出的多消费方
const TTL: Duration = Duration::from_secs(45);
const MAX_ENTRIES: usize = 24;

struct CacheEntry {
    value: Value,
    fetched_at: Instant,
}

fn cache() -> &'static Mutex<HashMap<u64, CacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(game_id: u64) -> Option<Value> {
    let map = cache().lock().unwrap_or_else(|e| e.into_inner());
    map.get(&game_id).and_then(|e| {
        if e.fetched_at.elapsed() < TTL {
            Some(e.value.clone())
        } else {
            None
        }
    })
}

fn cache_put(game_id: u64, value: Value) {
    let mut map = cache().lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, e| e.fetched_at.elapsed() < TTL);
    if map.len() >= MAX_ENTRIES {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, e)| e.fetched_at)
            .map(|(k, _)| *k)
        {
            map.remove(&oldest);
        }
    }
    map.insert(
        game_id,
        CacheEntry {
            value,
            fetched_at: Instant::now(),
        },
    );
}

/// 按 gameId 获取对局详情 JSON（进程内短 TTL 缓存）。
pub async fn fetch_match_detail_json(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    game_id: u64,
) -> Result<Value, String> {
    if let Some(hit) = cache_get(game_id) {
        log::debug!("对局 {} 详情命中缓存", game_id);
        crate::pipeline_stats::incr_match_detail_cache_hit();
        return Ok(hit);
    }
    crate::pipeline_stats::incr_match_detail_cache_miss();

    let url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
    let resp = http
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("获取对局详情失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("获取对局详情: HTTP {}", resp.status()));
    }
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析对局详情失败: {}", e))?;
    cache_put(game_id, value.clone());
    Ok(value)
}

/// 从 participant 节点提取非空 puuid
pub fn extract_puuid(participant: &Value) -> Option<String> {
    participant
        .get("puuid")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// 解析参与者队伍归属。
/// - Arena：`stats.subteamPlacement`（缺失则返回 None，避免误归队）
/// - 其他：`teamId` / `stats.teamId`，缺失时 0
pub fn resolve_team_id(participant: &Value, stats: &Value, is_arena: bool) -> Option<i32> {
    if is_arena {
        return stats
            .get("subteamPlacement")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
    }
    Some(
        participant
            .get("teamId")
            .and_then(|v| v.as_i64())
            .or_else(|| stats.get("teamId").and_then(|v| v.as_i64()))
            .unwrap_or(0) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cache_put_get_roundtrip() {
        cache_put(999001, json!({"gameId": 999001}));
        let hit = cache_get(999001).expect("应命中");
        assert_eq!(hit.get("gameId").and_then(|v| v.as_u64()), Some(999001));
    }

    #[test]
    fn cache_miss_returns_none() {
        assert!(cache_get(888777).is_none());
    }

    #[test]
    fn resolve_team_id_normal_and_arena() {
        let p = json!({"teamId": 100});
        let s = json!({"subteamPlacement": 2});
        assert_eq!(resolve_team_id(&p, &s, false), Some(100));
        assert_eq!(resolve_team_id(&p, &s, true), Some(2));
        assert_eq!(resolve_team_id(&p, &json!({}), true), None);
    }

    #[test]
    fn extract_puuid_requires_nonempty() {
        assert_eq!(
            extract_puuid(&json!({"puuid": "abc"})),
            Some("abc".to_string())
        );
        assert_eq!(extract_puuid(&json!({"puuid": ""})), None);
        assert_eq!(extract_puuid(&json!({})), None);
    }
}
