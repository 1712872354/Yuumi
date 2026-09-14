use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod augments;
pub mod data;
pub mod history;
pub mod rank;

pub use augments::get_tft_augments;
pub use data::{fetch_tft_meta_maps, get_tft_data};
pub use history::get_tft_match_history;
pub use rank::get_tft_ranked_stats;

pub(crate) static CACHED_TFT_DATA: RwLock<Option<TftDataMapping>> = RwLock::const_new(None);
pub(crate) static CACHED_AUGMENTS: RwLock<Option<Vec<TftAugmentInfo>>> = RwLock::const_new(None);
// TFT meta maps 的 Value 形态缓存：缓存命中时直接复用，避免反复重建五个映射
pub(crate) static CACHED_META_MAPS: RwLock<Option<Arc<serde_json::Value>>> =
    RwLock::const_new(None);

// ─── TFT 数据结构 ───

/// TFT 海克斯强化信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftAugmentInfo {
    pub api_name: String,
    pub name: String,
    pub desc: String,
    pub icon_path: String,
    pub tier: i32, // 1-银，2-金，3-彩/棱彩
}

/// TFT 解析后的资源映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TftDataMapping {
    pub champions: HashMap<String, String>,
    pub traits: HashMap<String, String>,
    pub champion_icons: HashMap<String, String>,
    pub trait_icons: HashMap<String, String>,
    pub item_icons: HashMap<String, String>,
    pub item_names: HashMap<String, String>,
}

/// TFT 段位信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftRankDisplay {
    pub solo_tier: String,
    pub solo_division: String,
    pub solo_lp: i32,
    pub solo_wins: i32,
    pub solo_losses: i32,
    pub turbo_tier: String,
    pub turbo_rating: i32,
    pub turbo_wins: i32,
    pub double_tier: String,
    pub double_division: String,
    pub double_lp: i32,
    pub double_wins: i32,
    pub double_losses: i32,
}

/// TFT 单个出战棋子数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftUnitDisplay {
    pub character_id: String,
    pub name: String,
    pub icon_url: String,
    pub rarity: i32,
    pub tier: i32, // 1-3星
    pub item_names: Vec<String>,
    pub item_icon_urls: Vec<String>,
}

/// TFT 单个激活羁绊数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftTraitDisplay {
    pub name: String,
    pub num_units: i32,
    pub tier_current: i32,
    pub icon_url: String,
}

/// TFT 单局中单个玩家的数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftParticipantDisplay {
    pub puuid: String,
    pub summoner_name: String,
    pub is_self: bool,
    pub placement: i32,
    pub level: i32,
    pub gold_left: i32,
    pub total_damage_to_players: i32,
    pub companion_icon_url: String,
    pub traits: Vec<TftTraitDisplay>,
    pub units: Vec<TftUnitDisplay>,
    pub augments: Vec<String>,
}

/// TFT 清洗后的单局战绩
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMatchDisplay {
    pub game_id: u64,
    pub queue_id: i32,
    pub queue_name: String,
    pub game_creation: u64,
    pub game_duration: u64,
    pub time_str: String,
    pub duration_str: String,
    pub placement: i32, // 1-8 名
    pub level: i32,
    pub gold_left: i32,
    pub total_damage_to_players: i32,
    pub companion_icon_url: String,
    pub traits: Vec<TftTraitDisplay>,
    pub units: Vec<TftUnitDisplay>,
    pub augments: Vec<String>,
    pub participants: Vec<TftParticipantDisplay>,
}

/// TFT 战绩汇总与统计
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMatchSummary {
    pub total_games: usize,
    pub win_count: usize,   // 登顶次数 (#1)
    pub top4_count: usize,  // 前四次数 (1-4)
    pub top4_rate: f64,     // 前四率 %
    pub win_rate: f64,      // 登顶率 %
    pub avg_placement: f64, // 平均名次
    pub matches: Vec<TftMatchDisplay>,
}

// ─── 共享辅助函数 ───

/// LCU 路径转换：把 .tex 换成 .png，低写处理并拼接 /lol-game-data/assets/
pub(crate) fn convert_lcu_icon_path(raw_icon: &str) -> String {
    if raw_icon.is_empty() {
        return String::new();
    }
    let mut path = raw_icon.to_lowercase().replace(".tex", ".png");

    if path.starts_with("/lol-game-data/") {
        // 已经包含 /lol-game-data/ 前缀
    } else if path.starts_with('/') {
        path = format!("/lol-game-data/assets{}", path);
    } else {
        path = format!("/lol-game-data/assets/{}", path);
    }

    path.replace("//", "/")
}

/// 将 LCU asset 路径转为 CDragon 直链
///
/// tft.json 里常见原始路径是 `ASSETS/Characters/.../*.tex`（无 `/lol-game-data` 前缀）。
/// 必须先归一化成 LCU 路径，再映射到 CDragon 的 `.../latest/game/...`，
/// 否则会生成 `.../latest/assets/...` 这种 404 URL。
pub(crate) fn convert_lcu_icon_path_to_cdragon(raw_icon: &str) -> String {
    if raw_icon.is_empty() {
        return String::new();
    }
    // 复用 LCU 路径归一化（补全 /lol-game-data/assets、.tex→.png、小写）
    let mut path = convert_lcu_icon_path(raw_icon);
    // 剔除文件名中的 Set 版本标记（如 `.TFT_Set17.tex`），该标记只存在于 tft.json 元数据，
    // CDragon 镜像与游戏文件实际不含此后缀，直接请求会 404
    strip_tft_set_marker_from_path(&mut path);
    if path.starts_with("/lol-game-data/assets/") {
        let sub = path.strip_prefix("/lol-game-data/assets/").unwrap_or(&path);
        format!("https://raw.communitydragon.org/latest/game/{}", sub)
    } else if path.starts_with("/lol-game-data/") {
        let sub = path.strip_prefix("/lol-game-data/").unwrap_or(&path);
        format!(
            "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/{}",
            sub
        )
    } else {
        // 兜底：仍走 game/ 前缀，避免再生成无效的 latest/<path>
        format!(
            "https://raw.communitydragon.org/latest/game/{}",
            path.trim_start_matches('/')
        )
    }
}

/// 剔除文件名中的 TFT Set 版本标记（形如 `.tft_set17` / `.tft_17_2` / `.tft_event_5yr_set17`），
/// 只处理最后一个 `/` 之后的文件名部分（目录名如 `tft17_riven` 无前导点，不会误匹配），
/// 且标记内必须包含数字，避免误伤正常文件名。
pub(crate) fn strip_tft_set_marker_from_path(path: &mut String) {
    let Some(slash) = path.rfind('/') else {
        return;
    };
    let head = slash + 1;
    let Some(rel) = path.get(head..) else {
        return;
    };
    let Some(marker) = rel.find(".tft_") else {
        return;
    };
    let marker_abs = head + marker;
    let Some(rest) = path.get(marker_abs + 1..) else {
        return;
    };
    let Some(ext_dot) = rest.find('.') else {
        return;
    };
    let token = &rest[..ext_dot];
    if !token.chars().any(|c| c.is_ascii_digit()) {
        return;
    }
    let ext = rest[ext_dot..].to_string();
    path.truncate(marker_abs);
    path.push_str(&ext);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tft_set_marker_from_path_strips_known_markers() {
        let cases = [
            (
                "/lol-game-data/assets/characters/tft17_riven/skins/base/images/tft17_riven_splash_tile_18.tft_set17.png",
                "/lol-game-data/assets/characters/tft17_riven/skins/base/images/tft17_riven_splash_tile_18.png",
            ),
            (
                "/lol-game-data/assets/characters/tft17_xxx/skins/base/images/xxx.tft_17_2.png",
                "/lol-game-data/assets/characters/tft17_xxx/skins/base/images/xxx.png",
            ),
            (
                "/lol-game-data/assets/characters/tft17_yyy/skins/base/images/yyy.tft_event_5yr_set17.png",
                "/lol-game-data/assets/characters/tft17_yyy/skins/base/images/yyy.png",
            ),
            (
                "/lol-game-data/assets/items/tft17_item_something.tft_set17_postlaunchaugments.png",
                "/lol-game-data/assets/items/tft17_item_something.png",
            ),
        ];
        for (input, expected) in cases {
            let mut path = input.to_string();
            strip_tft_set_marker_from_path(&mut path);
            assert_eq!(path, expected);
        }
    }

    #[test]
    fn strip_tft_set_marker_from_path_keeps_normal_paths() {
        let cases = [
            "/lol-game-data/assets/characters/tft16_aatrox/skins/base/images/tft16_aatrox_splash_tile_0.png",
            "/lol-game-data/assets/items/tft_item_bfs.png",
            "/lol-game-data/assets/characters/tft17_riven/skins/base/images/tft17_riven_splash_tile_18.png",
        ];
        for input in cases {
            let mut path = input.to_string();
            strip_tft_set_marker_from_path(&mut path);
            assert_eq!(path, input);
        }
    }
}
