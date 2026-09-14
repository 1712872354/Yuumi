use tauri::State;

use crate::{build_auth_header, AppState};

use super::TftRankDisplay;

/// 获取当前召唤师的云顶之弈段位数据
#[tauri::command]
pub async fn get_tft_ranked_stats(
    puuid: String,
    app_state: State<'_, AppState>,
) -> Result<TftRankDisplay, String> {
    // 尽早释放读锁，避免 HTTP 请求期间阻塞 monitor 重连
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client) = (lcu.port, lcu.token, lcu.http_client);

    let url = format!(
        "https://127.0.0.1:{}/lol-ranked/v1/ranked-stats/{}",
        port, puuid
    );
    let auth = build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("请求云顶段位失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取云顶段位失败: HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let mut rank_display = TftRankDisplay {
        solo_tier: "UNRANKED".to_string(),
        solo_division: "NA".to_string(),
        solo_lp: 0,
        solo_wins: 0,
        solo_losses: 0,
        turbo_tier: "NONE".to_string(),
        turbo_rating: 0,
        turbo_wins: 0,
        double_tier: "UNRANKED".to_string(),
        double_division: "NA".to_string(),
        double_lp: 0,
        double_wins: 0,
        double_losses: 0,
    };

    if let Some(queues) = json.get("queues").and_then(|q| q.as_array()) {
        for q in queues {
            let queue_type = q.get("queueType").and_then(|v| v.as_str()).unwrap_or("");
            match queue_type {
                "RANKED_TFT" => {
                    rank_display.solo_tier = q
                        .get("tier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNRANKED")
                        .to_string();
                    rank_display.solo_division = q
                        .get("division")
                        .or_else(|| q.get("rank"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("NA")
                        .to_string();
                    rank_display.solo_lp =
                        q.get("leaguePoints").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.solo_wins =
                        q.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.solo_losses =
                        q.get("losses").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                }
                "RANKED_TFT_TURBO" => {
                    rank_display.turbo_tier = q
                        .get("ratedTier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("NONE")
                        .to_string();
                    rank_display.turbo_rating =
                        q.get("ratedRating").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.turbo_wins =
                        q.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                }
                "RANKED_TFT_DOUBLE_UP" => {
                    rank_display.double_tier = q
                        .get("tier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNRANKED")
                        .to_string();
                    rank_display.double_division = q
                        .get("division")
                        .or_else(|| q.get("rank"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("NA")
                        .to_string();
                    rank_display.double_lp =
                        q.get("leaguePoints").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.double_wins =
                        q.get("wins").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    rank_display.double_losses =
                        q.get("losses").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                }
                _ => {}
            }
        }
    }

    Ok(rank_display)
}
