use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::AppState;

use super::types::{
    row_to_encountered, row_to_saved_player, EncounteredGameDto, PageResult, SavedPlayerDto,
    SAVED_PLAYER_COLS,
};
use super::{now, with_db};

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
