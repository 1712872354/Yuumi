use serde::{Deserialize, Serialize};

// ─── 数据结构 ───

/// 本局缓存：对局结束记录相遇时使用
#[derive(Debug, Clone)]
pub struct CurrentGameCache {
    pub game_id: i64,
    pub queue_id: i32,
    pub players: Vec<GamePlayerEntry>,
}

/// 单个玩家的对局信息
#[derive(Debug, Clone)]
pub struct GamePlayerEntry {
    pub puuid: String,
    pub summoner_name: String,
    pub profile_icon_id: i32,
    pub tag_line: Option<String>,
    pub champion_id: i32,
    /// "ally" / "enemy" / ""（未知）
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlayerDto {
    pub puuid: String,
    pub self_puuid: String,
    pub region: String,
    pub rso_platform_id: String,
    pub tag: Option<String>,
    pub summoner_name: String,
    pub profile_icon_id: i32,
    #[serde(default)]
    pub tag_line: Option<String>,
    #[serde(default)]
    pub champion_id: i32,
    pub update_at: i64,
    pub last_met_at: Option<i64>,
    #[serde(default)]
    pub last_queue_type: Option<String>,
    #[serde(default)]
    pub encounter_count: i32,
    /// 最近自动标签（大腿/坑/演员等）
    #[serde(default)]
    pub auto_tag: Option<String>,
    #[serde(default)]
    pub auto_score: Option<f64>,
    #[serde(default)]
    pub auto_reason: Option<String>,
    #[serde(default)]
    pub auto_tag_stats: Option<String>,
    /// 最近一次同局关系：ally / enemy
    #[serde(default)]
    pub last_relation: Option<String>,
    /// 名单类型：'' 普通 / 'black' 拉黑 / 'white' 加白
    #[serde(default)]
    pub list_kind: String,
    #[serde(default)]
    pub list_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounteredGameDto {
    pub id: i64,
    pub game_id: i64,
    pub puuid: String,
    pub self_puuid: String,
    pub region: String,
    pub rso_platform_id: String,
    pub queue_type: String,
    pub update_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult<T> {
    pub data: Vec<T>,
    pub count: i64,
}

pub(crate) fn row_to_saved_player(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedPlayerDto> {
    Ok(SavedPlayerDto {
        puuid: row.get(0)?,
        self_puuid: row.get(1)?,
        region: row.get(2)?,
        rso_platform_id: row.get(3)?,
        tag: row.get(4)?,
        summoner_name: row.get(5)?,
        profile_icon_id: row.get(6)?,
        tag_line: row.get(7)?,
        champion_id: row.get(8)?,
        update_at: row.get(9)?,
        last_met_at: row.get(10)?,
        auto_tag: row.get(11).ok().flatten(),
        auto_score: row.get(12).ok().flatten(),
        auto_reason: row.get(13).ok().flatten(),
        auto_tag_stats: row.get(14).ok().flatten(),
        last_relation: row.get(15).ok().flatten(),
        list_kind: row.get::<_, String>(16).unwrap_or_default(),
        list_reason: row.get(17).ok().flatten(),
        last_queue_type: row.get(18).ok().flatten(),
        encounter_count: row.get::<usize, i32>(19).unwrap_or(1),
    })
}

pub(crate) const SAVED_PLAYER_COLS: &str =
    "puuid, self_puuid, region, rso_platform_id, tag, summoner_name, profile_icon_id, tag_line, champion_id, update_at, last_met_at, auto_tag, auto_score, auto_reason, auto_tag_stats, last_relation, list_kind, list_reason";

pub(crate) fn row_to_encountered(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncounteredGameDto> {
    Ok(EncounteredGameDto {
        id: row.get(0)?,
        game_id: row.get(1)?,
        puuid: row.get(2)?,
        self_puuid: row.get(3)?,
        region: row.get(4)?,
        rso_platform_id: row.get(5)?,
        queue_type: row.get(6)?,
        update_at: row.get(7)?,
    })
}
