use serde::{Deserialize, Serialize};

pub(crate) mod display;
pub mod history;
pub mod teammates;

pub use history::{get_match_history, get_match_history_merged, get_match_history_sgp};
pub use queue_time::{is_arena_queue, is_tft_queue, queue_id_to_opgg_mode};
pub use teammates::{get_recent_teammates, RecentTeammate, RecentTeammatesResponse};

pub(crate) mod queue_time {
    use std::collections::HashMap;
    use std::sync::OnceLock;

    // ─── 队列元数据（与前端共用 src/shared/queue_meta.json 单一数据源） ───

    /// 与前端 `src/utils/queueMeta.ts` / `src/shared/queue_meta.json` 同源。
    const QUEUE_META_JSON: &str = include_str!("../../../../src/shared/queue_meta.json");

    #[derive(Debug, Clone, serde::Deserialize)]
    struct QueueMetaEntry {
        name: String,
        map: String,
    }

    #[derive(Debug, Clone, serde::Deserialize)]
    struct QueueMetaFile {
        #[serde(rename = "tftQueueIds")]
        tft_queue_ids: Vec<i32>,
        queues: HashMap<String, QueueMetaEntry>,
    }

    fn queue_meta() -> &'static QueueMetaFile {
        static META: OnceLock<QueueMetaFile> = OnceLock::new();
        META.get_or_init(|| {
            serde_json::from_str(QUEUE_META_JSON).expect("src/shared/queue_meta.json 解析失败")
        })
    }

    /// 斗魂竞技场队列（普通 1700 / 排位 1710），队伍归属应看 subteamPlacement 而非 teamId
    pub fn is_arena_queue(queue_id: i64) -> bool {
        matches!(queue_id, 1700 | 1710)
    }

    /// 云顶之弈队列（与前端 `TFT_QUEUE_IDS` 同源）
    pub fn is_tft_queue(queue_id: i32) -> bool {
        queue_meta().tft_queue_ids.contains(&queue_id)
    }

    /// 将 queueId 映射为 OP.GG 使用的游戏模式标识。
    /// 未知/自定义队列返回 None，由调用方自行推断，避免误请求 ranked。
    pub fn queue_id_to_opgg_mode(queue_id: i32) -> Option<&'static str> {
        match queue_id {
            450 | 2400 | 2450 => Some("aram"),
            1700 | 1710 => Some("arena"),
            1300 => Some("nexus_blitz"),
            900 | 1900 => Some("urf"),
            // 召唤师峡谷常见对战（含人机）按 ranked 路径
            400 | 420 | 430 | 440 | 480 | 490 | 800 | 810 | 830 | 840 | 850 => Some("ranked"),
            _ => None,
        }
    }

    pub(crate) struct QueueInfo {
        pub(crate) name: String,
        pub(crate) map: String,
    }

    pub(crate) fn get_queue_info(queue_id: i32) -> QueueInfo {
        match queue_meta().queues.get(&queue_id.to_string()) {
            Some(entry) => QueueInfo {
                name: entry.name.clone(),
                map: entry.map.clone(),
            },
            None => QueueInfo {
                name: "自定义模式".to_string(),
                map: "自定义".to_string(),
            },
        }
    }

    // ─── 时间工具函数 ───

    /// 毫秒时间戳 → 本地时区 "2024-01-15 20:30"
    pub(crate) fn timestamp_to_str(ms: u64) -> String {
        let secs = (ms / 1000) as i64;
        chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "1970-01-01 00:00".to_string())
    }

    /// 毫秒时间戳 → 本地时区 "01-15 20:30"
    pub(crate) fn timestamp_to_short_str(ms: u64) -> String {
        let secs = (ms / 1000) as i64;
        chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "01-01 00:00".to_string())
    }

    /// 秒数 → "25:30"
    pub(crate) fn secs_to_str(total_secs: u64) -> String {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}

// ─── LCU 原始响应结构体 ───

/// `/lol-match-history/v1/products/lol/{puuid}/matches` 的原始返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchHistoryResponse {
    pub games: LcuMatchGamesContainer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchGamesContainer {
    pub games: Vec<LcuMatchGame>,
    pub game_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchGame {
    pub game_id: u64,
    pub game_creation: u64,
    pub game_duration: u64,
    pub queue_id: i32,
    pub map_id: Option<u32>,
    pub participants: Vec<LcuMatchParticipant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchParticipant {
    #[serde(default)]
    pub champion_id: i32,
    #[serde(default)]
    pub spell1_id: i32,
    #[serde(default)]
    pub spell2_id: i32,
    pub stats: LcuMatchStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchStats {
    #[serde(default)]
    pub win: bool,
    #[serde(default)]
    pub kills: i32,
    #[serde(default)]
    pub deaths: i32,
    #[serde(default)]
    pub assists: i32,
    #[serde(default)]
    pub champ_level: i32,
    #[serde(default)]
    pub item0: i32,
    #[serde(default)]
    pub item1: i32,
    #[serde(default)]
    pub item2: i32,
    #[serde(default)]
    pub item3: i32,
    #[serde(default)]
    pub item4: i32,
    #[serde(default)]
    pub item5: i32,
    #[serde(default)]
    pub item6: i32,
    #[serde(default)]
    pub perk0: i32,
    pub total_minions_killed: Option<i32>,
    pub neutral_minions_killed: Option<i32>,
    pub gold_earned: Option<i32>,
    pub total_damage_dealt_to_champions: Option<i32>,
    pub total_damage_taken: Option<i32>,
    pub total_heal: Option<i32>,
    pub vision_score: Option<i32>,
    #[serde(default)]
    pub game_ended_in_early_surrender: bool,
    #[serde(default)]
    pub subteam_placement: Option<u32>,
    // 海克斯强化（海克斯大乱斗 queueId 2400 / 经典海斗 2450）
    #[serde(default)]
    pub augments: Vec<i32>,
    #[serde(default)]
    pub player_augment1: i32,
    #[serde(default)]
    pub player_augment2: i32,
    #[serde(default)]
    pub player_augment3: i32,
    #[serde(default)]
    pub player_augment4: i32,
    #[serde(default)]
    pub player_augment5: i32,
}

// ─── 前端展示用的清洗结构体 ───

/// 清洗后的单局战绩数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDisplay {
    pub queue_id: i32,
    pub game_id: u64,
    pub time: String,
    pub short_time: String,
    pub name: String,
    pub map: String,
    pub duration: String,
    pub remake: bool,
    pub win: bool,
    pub placement: Option<u32>,
    pub champion_id: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub champ_level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub kda: String,
    pub item_ids: Vec<i32>,
    pub rune_id: i32,
    pub cs: i32,
    pub gold: i32,
    pub time_stamp: u64,
    pub total_damage: i32,
    pub total_damage_taken: i32,
    pub total_heal: i32,
    pub vision_score: i32,
    // 前端拼接图标的 URL 前缀
    pub champion_icon_url: String,
    pub spell1_icon_url: String,
    pub spell2_icon_url: String,
    pub rune_icon_url: String,
    pub item_icon_urls: Vec<String>,
    // 海克斯强化（仅海克斯大乱斗 2400 / 经典海斗 2450 有值）
    pub augment_ids: Vec<i32>,
    pub augment_icon_urls: Vec<String>,
    pub augment_names: Vec<String>,
}
