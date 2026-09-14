//! 对局结束自动打标：根据本局表现给极端玩家打「大腿 / 坑 / 演员 / 躺赢」等标签。
//! 只打极端标签，避免人人有标；不覆盖用户手动 tag。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 自动标签枚举（与前端展示一一对应）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoTag {
    /// 大腿：胜 + 高 KDA + 输出突出
    Carry,
    /// C 位：全队输出第一且 KDA 尚可
    CarryDamage,
    /// 坑：败 + 高死亡 + 低 KDA + 输出垫底
    Feeder,
    /// 演员：极端死亡 + 伤转极低
    Inter,
    /// 躺赢：胜但 KDA/输出/视野均垫底
    Carried,
}

impl AutoTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            AutoTag::Carry => "大腿",
            AutoTag::CarryDamage => "C位",
            AutoTag::Feeder => "坑",
            AutoTag::Inter => "演员",
            AutoTag::Carried => "躺赢",
        }
    }

    pub fn as_key(&self) -> &'static str {
        match self {
            AutoTag::Carry => "carry",
            AutoTag::CarryDamage => "carryDamage",
            AutoTag::Feeder => "feeder",
            AutoTag::Inter => "inter",
            AutoTag::Carried => "carried",
        }
    }
}

/// 单人本局表现快照（用于相对比较）
#[derive(Debug, Clone)]
pub struct PlayerPerf {
    pub puuid: String,
    pub champion_id: i32,
    pub team_id: i32,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub damage: i32,
    pub damage_taken: i32,
    pub vision: i32,
    pub cs: i32,
    pub gold: i32,
    pub remake: bool,
}

impl PlayerPerf {
    pub fn kda(&self) -> f64 {
        if self.deaths == 0 {
            (self.kills + self.assists) as f64
        } else {
            (self.kills + self.assists) as f64 / self.deaths as f64
        }
    }

    /// 伤转：输出 / 承伤（%），承伤为 0 时返回 0
    pub fn damage_ratio(&self) -> f64 {
        if self.damage_taken <= 0 {
            0.0
        } else {
            self.damage as f64 / self.damage_taken as f64 * 100.0
        }
    }
}

/// 打标结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTagResult {
    pub puuid: String,
    pub tag: AutoTag,
    pub score: f64,
    pub reason: String,
}

/// 敏感度：0 严格 / 1 标准 / 2 宽松
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    Strict,
    Normal,
    Loose,
}

impl Sensitivity {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => Sensitivity::Strict,
            2 => Sensitivity::Loose,
            _ => Sensitivity::Normal,
        }
    }

    fn scale(&self) -> f64 {
        match self {
            Sensitivity::Strict => 1.15, // 阈值更严
            Sensitivity::Normal => 1.0,
            Sensitivity::Loose => 0.88,
        }
    }
}

/// 综合表现分（0-100），便于排序/展示
fn performance_score(p: &PerfContext) -> f64 {
    let kda_n = (p.kda / 8.0).clamp(0.0, 1.0);
    let dmg_n = (p.damage_share / 0.35).clamp(0.0, 1.0);
    let vis_n = (p.vision as f64 / 40.0).clamp(0.0, 1.0);
    let win_bonus = if p.win { 0.1 } else { 0.0 };
    ((kda_n * 0.4 + dmg_n * 0.35 + vis_n * 0.15 + win_bonus) * 100.0).round()
}

struct PerfContext {
    puuid: String,
    win: bool,
    kills: i32,
    deaths: i32,
    assists: i32,
    kda: f64,
    damage_share: f64,
    damage_ratio: f64,
    vision: i32,
    /// 队内伤害数据是否可用（全 0 时为 false，避免误判垫底/占比）
    damage_stats_valid: bool,
    is_team_top_damage: bool,
    is_team_bottom_damage: bool,
    is_team_bottom_vision: bool,
    is_team_bottom_kda: bool,
}

fn build_context(players: &[PlayerPerf]) -> Vec<PerfContext> {
    let mut teams: std::collections::HashMap<i32, Vec<&PlayerPerf>> = Default::default();
    for p in players {
        teams.entry(p.team_id).or_default().push(p);
    }

    players
        .iter()
        .map(|p| {
            let team = teams.get(&p.team_id).cloned().unwrap_or_default();
            let team_damage_total: f64 = team.iter().map(|x| x.damage as f64).sum();
            let team_taken_total: f64 = team.iter().map(|x| x.damage_taken as f64).sum();
            // 全员伤害/承伤均为 0 时视为数据缺失，不再用 max(1.0) 放大占比或垫底
            let damage_stats_valid = team_damage_total > 0.0 || team_taken_total > 0.0;
            let team_damages: Vec<f64> = team.iter().map(|x| x.damage as f64).collect();
            let team_visions: Vec<f64> = team.iter().map(|x| x.vision as f64).collect();
            let team_kdas: Vec<f64> = team.iter().map(|x| x.kda()).collect();

            let max_damage = team_damages.iter().cloned().fold(0.0_f64, f64::max);
            let min_damage = team_damages.iter().cloned().fold(f64::MAX, f64::min);
            let min_vision = team_visions.iter().cloned().fold(f64::MAX, f64::min);
            let min_kda = team_kdas.iter().cloned().fold(f64::MAX, f64::min);

            PerfContext {
                puuid: p.puuid.clone(),
                win: p.win,
                kills: p.kills,
                deaths: p.deaths,
                assists: p.assists,
                kda: p.kda(),
                damage_share: if damage_stats_valid {
                    p.damage as f64 / team_damage_total.max(1.0)
                } else {
                    0.0
                },
                damage_ratio: p.damage_ratio(),
                vision: p.vision,
                damage_stats_valid,
                is_team_top_damage: damage_stats_valid && p.damage as f64 >= max_damage - 1.0,
                is_team_bottom_damage: damage_stats_valid && p.damage as f64 <= min_damage + 1.0,
                is_team_bottom_vision: p.vision as f64 <= min_vision + 0.5,
                is_team_bottom_kda: p.kda() <= min_kda + 0.05,
            }
        })
        .collect()
}

/// 对一局全部玩家做极端打标。返回有标签的玩家（通常 0~4 人）。
pub fn evaluate_match(players: &[PlayerPerf], sensitivity: Sensitivity) -> Vec<AutoTagResult> {
    // 重开 / 人数异常不打标
    if players.len() < 4 || players.iter().any(|p| p.remake) {
        return Vec::new();
    }

    let s = sensitivity.scale();
    let ctx = build_context(players);
    let mut results = Vec::new();

    for p in &ctx {
        let mut tags: Vec<(AutoTag, f64, String)> = Vec::new();

        // ── 大腿：赢 + 高 KDA + 输出突出 ──
        let carry_kda = 4.0 * s;
        let carry_share = 0.28 * s;
        if p.win && p.kda >= carry_kda && p.damage_share >= carry_share {
            tags.push((
                AutoTag::Carry,
                performance_score(p),
                format!(
                    "胜 · KDA {:.1} · 输出占比 {:.0}%",
                    p.kda,
                    p.damage_share * 100.0
                ),
            ));
        }

        // ── C 位：全队输出第一 + KDA≥2.2 ──
        let c_kda = 2.2 * s;
        if p.win && p.is_team_top_damage && p.kda >= c_kda && p.damage_share >= 0.22 {
            tags.push((
                AutoTag::CarryDamage,
                performance_score(p),
                format!("全队输出第一 · 占比 {:.0}%", p.damage_share * 100.0),
            ));
        }

        // ── 坑：败 + 高死亡 + 低 KDA + 输出垫底 ──
        let feeder_deaths = (10.0 / s).round() as i32;
        let feeder_kda = 1.5 / s;
        if !p.win && p.deaths >= feeder_deaths && p.kda < feeder_kda && p.is_team_bottom_damage {
            tags.push((
                AutoTag::Feeder,
                performance_score(p),
                format!("败 · {}死 · KDA {:.1} · 输出垫底", p.deaths, p.kda),
            ));
        }

        // ── 演员：极端死亡 + 伤转极低（伤害数据缺失时不打） ──
        let inter_deaths = (12.0 / s).round() as i32;
        let inter_ratio = 55.0 * s;
        if p.damage_stats_valid
            && p.deaths >= inter_deaths
            && p.damage_ratio < inter_ratio
            && p.kills + p.assists <= 6
        {
            tags.push((
                AutoTag::Inter,
                performance_score(p),
                format!(
                    "{}死 · 伤转 {:.0}% · 参与 {}",
                    p.deaths,
                    p.damage_ratio,
                    p.kills + p.assists
                ),
            ));
        }

        // ── 躺赢：赢但 KDA/输出/视野均垫底 ──
        let carried_kda = 2.0 / s;
        if p.win
            && p.kda < carried_kda
            && p.is_team_bottom_damage
            && p.is_team_bottom_vision
            && p.is_team_bottom_kda
        {
            tags.push((
                AutoTag::Carried,
                performance_score(p),
                format!("胜 · KDA {:.1} · 输出/视野垫底", p.kda),
            ));
        }

        // 同一人只保留优先级最高的一项：大腿 > 演员 > 坑 > C位 > 躺赢
        if let Some(best) = pick_primary_tag(tags) {
            results.push(AutoTagResult {
                puuid: p.puuid.clone(),
                tag: best.0,
                score: best.1,
                reason: best.2,
            });
        }
    }

    results
}

fn pick_primary_tag(mut tags: Vec<(AutoTag, f64, String)>) -> Option<(AutoTag, f64, String)> {
    let priority = |t: AutoTag| match t {
        AutoTag::Carry => 0,
        AutoTag::Inter => 1,
        AutoTag::Feeder => 2,
        AutoTag::CarryDamage => 3,
        AutoTag::Carried => 4,
    };
    tags.sort_by_key(|(t, _, _)| priority(*t));
    tags.into_iter().next()
}

/// 从 LCU/SGP 原始 JSON 解析本局全部参与者表现。
/// 斗魂竞技场（1700/1710）队伍归属用 `stats.subteamPlacement`，其余用 `teamId`。
pub fn parse_participants_from_match_json(game: &Value) -> Vec<PlayerPerf> {
    let Some(list) = game.get("participants").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let queue_id = game.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
    let is_arena = crate::parsers::match_parser::is_arena_queue(queue_id);

    let remake = list.iter().any(|p| {
        p.get("stats")
            .and_then(|s| s.get("gameEndedInEarlySurrender"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    });

    list.iter()
        .filter_map(|p| {
            let stats = p.get("stats")?;
            let puuid = crate::lcu::match_detail::extract_puuid(p)?;
            let team_id = crate::lcu::match_detail::resolve_team_id(p, stats, is_arena)?;

            let dmg = stats
                .get("totalDamageDealtToChampions")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let taken = stats
                .get("totalDamageTaken")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            Some(PlayerPerf {
                puuid,
                champion_id: p.get("championId").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                team_id,
                win: stats.get("win").and_then(|v| v.as_bool()).unwrap_or(false),
                kills: stats.get("kills").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                deaths: stats.get("deaths").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                assists: stats.get("assists").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                damage: dmg,
                damage_taken: taken,
                vision: stats
                    .get("visionScore")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32,
                cs: stats
                    .get("totalMinionsKilled")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32
                    + stats
                        .get("neutralMinionsKilled")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32,
                gold: stats
                    .get("goldEarned")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32,
                remake,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(puuid: &str, team: i32, win: bool) -> PlayerPerf {
        PlayerPerf {
            puuid: puuid.into(),
            champion_id: 1,
            team_id: team,
            win,
            kills: 5,
            deaths: 5,
            assists: 5,
            damage: 15000,
            damage_taken: 15000,
            vision: 20,
            cs: 150,
            gold: 12000,
            remake: false,
        }
    }

    fn fill_teams() -> Vec<PlayerPerf> {
        // 队伍 100 胜，队伍 200 负，各 5 人
        let mut v = Vec::new();
        for i in 0..5 {
            let mut p = base(&format!("win{i}"), 100, true);
            p.damage = 12000 + i * 1000;
            v.push(p);
        }
        for i in 0..5 {
            let mut p = base(&format!("lose{i}"), 200, false);
            p.damage = 10000 + i * 800;
            v.push(p);
        }
        v
    }

    #[test]
    fn remake_game_tags_nothing() {
        let mut players = fill_teams();
        players[0].remake = true;
        assert!(evaluate_match(&players, Sensitivity::Normal).is_empty());
    }

    #[test]
    fn carry_tagged_on_strong_win() {
        let mut players = fill_teams();
        // 超神 + 最高输出
        players[0].kills = 12;
        players[0].deaths = 1;
        players[0].assists = 8;
        players[0].damage = 32000;
        let tags = evaluate_match(&players, Sensitivity::Normal);
        let carry = tags.iter().find(|t| t.tag == AutoTag::Carry);
        assert!(carry.is_some(), "应打出大腿标签: {:?}", tags);
        assert_eq!(carry.unwrap().puuid, "win0");
    }

    #[test]
    fn feeder_or_inter_tagged_on_heavy_loss() {
        let mut players = fill_teams();
        players[5].kills = 1;
        players[5].deaths = 12;
        players[5].assists = 2;
        players[5].damage = 6000;
        players[5].damage_taken = 28000;
        let tags = evaluate_match(&players, Sensitivity::Normal);
        let hit = tags
            .iter()
            .find(|t| t.puuid == "lose0" && (t.tag == AutoTag::Feeder || t.tag == AutoTag::Inter));
        assert!(hit.is_some(), "应打出坑/演员标签: {:?}", tags);
    }

    #[test]
    fn normal_players_untagged() {
        let players = fill_teams();
        let tags = evaluate_match(&players, Sensitivity::Normal);
        assert!(tags.is_empty(), "普通表现不应打标: {:?}", tags);
    }

    #[test]
    fn parse_participants_reads_puuid_and_stats() {
        let json = serde_json::json!({
            "participants": [
                {
                    "puuid": "aaa",
                    "championId": 99,
                    "teamId": 100,
                    "stats": {
                        "win": true, "kills": 3, "deaths": 1, "assists": 4,
                        "totalDamageDealtToChampions": 20000,
                        "totalDamageTaken": 10000,
                        "visionScore": 15,
                        "totalMinionsKilled": 100,
                        "neutralMinionsKilled": 20,
                        "goldEarned": 11000
                    }
                }
            ]
        });
        let list = parse_participants_from_match_json(&json);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].puuid, "aaa");
        assert!(list[0].win);
        assert_eq!(list[0].cs, 120);
        assert!((list[0].damage_ratio() - 200.0).abs() < 0.1);
    }

    #[test]
    fn parse_arena_uses_subteam_placement() {
        let json = serde_json::json!({
            "queueId": 1700,
            "participants": [
                {
                    "puuid": "a",
                    "championId": 1,
                    "teamId": 100,
                    "stats": {
                        "subteamPlacement": 1,
                        "win": true, "kills": 5, "deaths": 1, "assists": 5,
                        "totalDamageDealtToChampions": 20000,
                        "totalDamageTaken": 10000
                    }
                },
                {
                    "puuid": "b",
                    "championId": 2,
                    "teamId": 100,
                    "stats": {
                        "subteamPlacement": 2,
                        "win": false, "kills": 1, "deaths": 8, "assists": 2,
                        "totalDamageDealtToChampions": 8000,
                        "totalDamageTaken": 20000
                    }
                },
                {
                    "puuid": "missing-placement",
                    "championId": 3,
                    "teamId": 100,
                    "stats": {
                        "win": true, "kills": 3, "deaths": 2, "assists": 3,
                        "totalDamageDealtToChampions": 12000,
                        "totalDamageTaken": 12000
                    }
                }
            ]
        });
        let list = parse_participants_from_match_json(&json);
        assert_eq!(
            list.len(),
            2,
            "Arena 缺 subteamPlacement 应跳过: {:?}",
            list
        );
        let a = list.iter().find(|p| p.puuid == "a").unwrap();
        let b = list.iter().find(|p| p.puuid == "b").unwrap();
        assert_eq!(a.team_id, 1);
        assert_eq!(b.team_id, 2);
    }

    #[test]
    fn zero_damage_team_skips_damage_tags() {
        let mut players = fill_teams();
        for p in &mut players {
            p.damage = 0;
            p.damage_taken = 0;
            if !p.win {
                p.deaths = 14;
                p.kills = 0;
                p.assists = 1;
            }
        }
        // 伤害数据全 0 时不应打出坑/演员（依赖输出垫底/伤转）
        let tags = evaluate_match(&players, Sensitivity::Normal);
        assert!(
            tags.iter().all(|t| t.tag != AutoTag::Feeder
                && t.tag != AutoTag::Inter
                && t.tag != AutoTag::Carried),
            "伤害全 0 不应打伤害相关标: {:?}",
            tags
        );
    }
}
