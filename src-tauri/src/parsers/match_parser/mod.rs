use serde::{Deserialize, Serialize};

pub(crate) mod display;
pub mod history;
pub mod teammates;

pub use history::{get_match_history, get_match_history_merged, get_match_history_sgp};
pub use queue_time::{is_arena_queue, queue_id_to_opgg_mode};
pub use teammates::{get_recent_teammates, RecentTeammate, RecentTeammatesResponse};

pub(crate) mod queue_time {
    // ─── 队列 ID 映射 ───

    /// 斗魂竞技场队列（普通 1700 / 排位 1710），队伍归属应看 subteamPlacement 而非 teamId
    pub fn is_arena_queue(queue_id: i64) -> bool {
        matches!(queue_id, 1700 | 1710)
    }

    /// 将 queueId 映射为 OP.GG 使用的游戏模式标识（供自动选人与其他调用方复用）
    pub fn queue_id_to_opgg_mode(queue_id: i32) -> &'static str {
        match queue_id {
            450 | 2400 | 2450 => "aram",
            1700 | 1710 => "arena",
            1300 => "nexus_blitz",
            900 | 1900 => "urf",
            _ => "ranked",
        }
    }

    pub(crate) struct QueueInfo {
        pub(crate) name: &'static str,
        pub(crate) map: &'static str,
    }

    pub(crate) fn get_queue_info(queue_id: i32) -> QueueInfo {
        match queue_id {
            // 召唤师峡谷
            400 => QueueInfo {
                name: "征召模式",
                map: "召唤师峡谷",
            },
            420 => QueueInfo {
                name: "排位单双排",
                map: "召唤师峡谷",
            },
            430 => QueueInfo {
                name: "匹配模式",
                map: "召唤师峡谷",
            },
            440 => QueueInfo {
                name: "排位灵活组排",
                map: "召唤师峡谷",
            },
            480 => QueueInfo {
                name: "快速模式",
                map: "召唤师峡谷",
            },
            490 => QueueInfo {
                name: "快速模式",
                map: "召唤师峡谷",
            },
            // 嚎哭深渊
            450 => QueueInfo {
                name: "极地大乱斗",
                map: "嚎哭深渊",
            },
            // 海克斯大乱斗
            2400 => QueueInfo {
                name: "海克斯大乱斗",
                map: "嚎哭深渊",
            },
            // 经典海斗 (Classic Hextech ARAM / KIWI_JADE)
            2450 => QueueInfo {
                name: "经典海斗",
                map: "嚎哭深渊",
            },
            // 限时/特殊模式
            800 => QueueInfo {
                name: "人机对战",
                map: "召唤师峡谷",
            },
            810 => QueueInfo {
                name: "人机对战",
                map: "召唤师峡谷",
            },
            820 => QueueInfo {
                name: "人机对战",
                map: "嚎哭深渊",
            },
            830 => QueueInfo {
                name: "人机对战",
                map: "召唤师峡谷",
            },
            840 => QueueInfo {
                name: "人机对战",
                map: "召唤师峡谷",
            },
            850 => QueueInfo {
                name: "人机对战",
                map: "召唤师峡谷",
            },
            900 => QueueInfo {
                name: "无限火力",
                map: "召唤师峡谷",
            },
            1010 => QueueInfo {
                name: "随机无限火力",
                map: "嚎哭深渊",
            },
            1020 => QueueInfo {
                name: "克隆模式",
                map: "召唤师峡谷",
            },
            1300 => QueueInfo {
                name: "极限闪击",
                map: "极限闪击",
            },
            1700 => QueueInfo {
                name: "斗魂竞技场",
                map: "斗魂竞技场",
            },
            1710 => QueueInfo {
                name: "斗魂竞技场",
                map: "斗魂竞技场",
            },
            // 捉鬼模式 (Swarm)
            1810 => QueueInfo {
                name: "捉鬼模式",
                map: "捉鬼模式",
            },
            1820 => QueueInfo {
                name: "捉鬼模式",
                map: "捉鬼模式",
            },
            1830 => QueueInfo {
                name: "捉鬼模式",
                map: "捉鬼模式",
            },
            1840 => QueueInfo {
                name: "捉鬼模式",
                map: "捉鬼模式",
            },
            // 经典模式 (League Classic)
            4300 => QueueInfo {
                name: "经典模式",
                map: "召唤师峡谷",
            },
            4310 => QueueInfo {
                name: "经典模式",
                map: "召唤师峡谷",
            },
            // 自定义
            0 => QueueInfo {
                name: "自定义模式",
                map: "自定义",
            },
            _ => QueueInfo {
                name: "自定义模式",
                map: "自定义",
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
