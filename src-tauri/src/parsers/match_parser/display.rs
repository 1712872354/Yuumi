use super::queue_time::{get_queue_info, secs_to_str, timestamp_to_short_str, timestamp_to_str};
use super::{LcuMatchGame, LcuMatchStats, MatchDisplay};

// ─── 数据清洗 ───

/// 从 stats 中提取海克斯强化 ID，去重后最多返回 5 个。
/// 将候选海克斯强化 ID 去重、过滤 0、最多保留 5 个
pub(crate) fn dedupe_augment_ids<I: IntoIterator<Item = i32>>(ids: I) -> Vec<i32> {
    let mut seen = std::collections::HashSet::new();
    let mut ordered = Vec::new();
    for id in ids {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    ordered.truncate(5);
    ordered
}

/// 从 stats 中提取海克斯强化 ID（去重，最多 5 个）
fn extract_augment_ids(stats: &LcuMatchStats) -> Vec<i32> {
    dedupe_augment_ids(stats.augments.iter().copied().chain([
        stats.player_augment1,
        stats.player_augment2,
        stats.player_augment3,
        stats.player_augment4,
        stats.player_augment5,
    ]))
}

/// 根据 ID 列表从资源表解析海克斯图标/名称（名称为空时兜底"海克斯强化"）
pub(crate) fn resolve_augment_details(
    ids: &[i32],
    assets: &crate::lcu::game_data::GameDataAssets,
) -> (Vec<String>, Vec<String>) {
    let mut icon_urls = Vec::new();
    let mut names = Vec::new();
    for &id in ids {
        if let Some(detail) = assets.augments.get(&id) {
            icon_urls.push(detail.icon_path.clone());
            let name = if detail.name.trim().is_empty() {
                "海克斯强化".to_string()
            } else {
                detail.name.clone()
            };
            names.push(name);
        }
    }
    (icon_urls, names)
}

impl LcuMatchGame {
    /// 将 LCU 原始对局数据清洗为前端展示结构。
    /// 无参与者时返回 None（不依赖调用方先做非空过滤，杜绝索引越界）。
    pub fn to_display(
        &self,
        assets: &crate::lcu::game_data::GameDataAssets,
    ) -> Option<MatchDisplay> {
        let participant = self.participants.first()?;
        let stats = &participant.stats;

        let cs =
            stats.total_minions_killed.unwrap_or(0) + stats.neutral_minions_killed.unwrap_or(0);
        let gold = stats.gold_earned.unwrap_or(0);
        let total_damage = stats.total_damage_dealt_to_champions.unwrap_or(0);
        let total_damage_taken = stats.total_damage_taken.unwrap_or(0);
        let total_heal = stats.total_heal.unwrap_or(0);
        let vision_score = stats.vision_score.unwrap_or(0);

        let item_ids = vec![
            stats.item0,
            stats.item1,
            stats.item2,
            stats.item3,
            stats.item4,
            stats.item5,
            stats.item6,
        ];

        let queue_info = get_queue_info(self.queue_id);
        let time = timestamp_to_str(self.game_creation);
        let short_time = timestamp_to_short_str(self.game_creation);
        let duration = secs_to_str(self.game_duration);

        let kda = if stats.deaths == 0 {
            "Perfect".to_string()
        } else {
            format!(
                "{:.2}",
                (stats.kills as f64 + stats.assists as f64) / stats.deaths as f64
            )
        };

        let champion_icon_url = format!(
            "/lol-game-data/assets/v1/champion-icons/{}.png",
            participant.champion_id
        );
        let spell1_icon_url = assets
            .spells
            .get(&participant.spell1_id)
            .cloned()
            .unwrap_or_default();
        let spell2_icon_url = assets
            .spells
            .get(&participant.spell2_id)
            .cloned()
            .unwrap_or_default();
        let rune_icon_url = assets.runes.get(&stats.perk0).cloned().unwrap_or_default();
        let item_icon_urls: Vec<String> = item_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter_map(|id| assets.items.get(id).cloned())
            .collect();

        let augment_ids = extract_augment_ids(stats);
        let (augment_icon_urls, augment_names) = resolve_augment_details(&augment_ids, assets);

        Some(MatchDisplay {
            queue_id: self.queue_id,
            game_id: self.game_id,
            time,
            short_time,
            name: queue_info.name,
            map: queue_info.map,
            duration,
            remake: stats.game_ended_in_early_surrender,
            win: stats.win,
            placement: stats.subteam_placement,
            champion_id: participant.champion_id,
            spell1_id: participant.spell1_id,
            spell2_id: participant.spell2_id,
            champ_level: stats.champ_level,
            kills: stats.kills,
            deaths: stats.deaths,
            assists: stats.assists,
            kda,
            item_ids,
            rune_id: stats.perk0,
            cs,
            gold,
            time_stamp: self.game_creation,
            total_damage,
            total_damage_taken,
            total_heal,
            vision_score,
            champion_icon_url,
            spell1_icon_url,
            spell2_icon_url,
            rune_icon_url,
            item_icon_urls,
            augment_ids,
            augment_icon_urls,
            augment_names,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcu::game_data::{CherryAugmentDetail, GameDataAssets};
    use crate::parsers::match_parser::{is_arena_queue, is_tft_queue, queue_id_to_opgg_mode};
    use serde_json::json;

    #[test]
    fn dedupe_augment_ids_filters_zero_and_dedupes_in_order() {
        assert_eq!(dedupe_augment_ids([3, 0, 1, 3, 2, 1]), vec![3, 1, 2]);
        assert!(dedupe_augment_ids(std::iter::empty()).is_empty());
    }

    #[test]
    fn dedupe_augment_ids_truncates_to_five() {
        assert_eq!(
            dedupe_augment_ids([1, 2, 3, 4, 5, 6, 7]),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn extract_augment_ids_merges_augments_and_player_slots() {
        let stats: LcuMatchStats = serde_json::from_value(json!({
            "win": true, "kills": 0, "deaths": 0, "assists": 0,
            "champLevel": 18,
            "item0": 0, "item1": 0, "item2": 0, "item3": 0,
            "item4": 0, "item5": 0, "item6": 0, "perk0": 0,
            "augments": [7010, 0],
            "playerAugment1": 7018,
            "playerAugment2": 7027,
            "playerAugment3": 7010,
            "playerAugment4": 0,
            "playerAugment5": 7022
        }))
        .unwrap();
        assert_eq!(extract_augment_ids(&stats), vec![7010, 7018, 7027, 7022]);
    }

    #[test]
    fn queue_id_to_opgg_mode_maps_known_queues() {
        assert_eq!(queue_id_to_opgg_mode(450), Some("aram"));
        assert_eq!(queue_id_to_opgg_mode(2400), Some("aram"));
        assert_eq!(queue_id_to_opgg_mode(1700), Some("arena"));
        assert_eq!(queue_id_to_opgg_mode(1300), Some("nexus_blitz"));
        assert_eq!(queue_id_to_opgg_mode(900), Some("urf"));
        assert_eq!(queue_id_to_opgg_mode(420), Some("ranked"));
        assert_eq!(queue_id_to_opgg_mode(-1), None);
        assert_eq!(queue_id_to_opgg_mode(123456), None);
    }

    #[test]
    fn is_arena_queue_covers_normal_and_ranked() {
        assert!(is_arena_queue(1700));
        assert!(is_arena_queue(1710));
        assert!(!is_arena_queue(420));
        assert!(!is_arena_queue(0));
    }

    #[test]
    fn is_tft_queue_covers_known_modes() {
        assert!(is_tft_queue(1090));
        assert!(is_tft_queue(1100));
        assert!(is_tft_queue(1130));
        assert!(is_tft_queue(1160));
        assert!(!is_tft_queue(420));
        assert!(!is_tft_queue(0));
    }

    fn sample_game(queue_id: i32) -> LcuMatchGame {
        serde_json::from_value(json!({
            "gameId": 483_920_751_u64,
            "gameCreation": 1_705_329_000_000_u64,
            "gameDuration": 1530_u64,
            "queueId": queue_id,
            "participants": [{
                "championId": 432,
                "spell1Id": 4,
                "spell2Id": 12,
                "stats": {
                    "win": true,
                    "kills": 10, "deaths": 2, "assists": 8,
                    "champLevel": 18,
                    "item0": 3157, "item1": 3020, "item2": 0, "item3": 0,
                    "item4": 0, "item5": 0, "item6": 0,
                    "perk0": 8112,
                    "totalMinionsKilled": 20,
                    "neutralMinionsKilled": 12,
                    "goldEarned": 13500,
                    "totalDamageDealtToChampions": 28000,
                    "totalHeal": 1200
                }
            }]
        }))
        .unwrap()
    }

    #[test]
    fn to_display_cleans_core_stats() {
        let mut assets = GameDataAssets::default();
        assets
            .spells
            .insert(4, "/lol-game-data/assets/spell/Summoner_Flash.png".into());
        assets
            .items
            .insert(3157, "/lol-game-data/assets/items/item_3157.png".into());
        assets.augments.insert(
            7018,
            CherryAugmentDetail {
                id: 7018,
                name: "".into(),
                icon_path: "/fe/lol-loot/aug_7018.png".into(),
            },
        );

        let mut game = sample_game(450);
        game.participants[0].stats.player_augment1 = 7018;

        let d = game.to_display(&assets).unwrap();
        assert_eq!(d.name, "极地大乱斗");
        assert_eq!(d.map, "嚎哭深渊");
        let expected_time = chrono::DateTime::from_timestamp(1_705_329_000, 0)
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string();
        assert_eq!(d.time, expected_time);
        assert_eq!(d.duration, "25:30");
        assert!(d.win);
        assert!(!d.remake);
        assert_eq!(d.kda, "9.00");
        assert_eq!(d.cs, 32);
        assert_eq!(d.gold, 13500);
        assert_eq!(
            d.champion_icon_url,
            "/lol-game-data/assets/v1/champion-icons/432.png"
        );
        // 物品图标：仅 item0 有效（>0 且在资源表中）
        assert_eq!(d.item_icon_urls.len(), 1);
        assert_eq!(
            d.spell1_icon_url,
            "/lol-game-data/assets/spell/Summoner_Flash.png"
        );
        // 海克斯强化：名称为空时兜底
        assert_eq!(d.augment_names, vec!["海克斯强化"]);
        assert_eq!(d.augment_icon_urls, vec!["/fe/lol-loot/aug_7018.png"]);
    }

    #[test]
    fn to_display_perfect_kda_when_zero_deaths() {
        let mut game = sample_game(420);
        game.participants[0].stats.deaths = 0;
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.kda, "Perfect");
        assert_eq!(d.name, "排位单双排");
        assert_eq!(d.map, "召唤师峡谷");
    }

    #[test]
    fn to_display_returns_none_without_participants() {
        let mut game = sample_game(420);
        game.participants.clear();
        assert!(game.to_display(&GameDataAssets::default()).is_none());
    }

    #[test]
    fn secs_to_str_formats_minutes_and_seconds() {
        assert_eq!(secs_to_str(1530), "25:30");
        assert_eq!(secs_to_str(3725), "62:05"); // 不进位小时
        assert_eq!(secs_to_str(59), "00:59");
    }

    #[test]
    fn timestamp_helpers_format_local() {
        // 1705329000000 = 2024-01-15 14:30:00 UTC；期望按本机时区展示
        let expected = chrono::DateTime::from_timestamp(1_705_329_000, 0)
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string();
        assert_eq!(timestamp_to_str(1_705_329_000_000), expected);

        let expected_short = chrono::DateTime::from_timestamp(1_705_329_000, 0)
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%m-%d %H:%M")
            .to_string();
        assert_eq!(timestamp_to_short_str(1_705_329_000_000), expected_short);

        // 无效时间戳兜底
        assert_eq!(timestamp_to_str(u64::MAX), "1970-01-01 00:00");
    }
}
