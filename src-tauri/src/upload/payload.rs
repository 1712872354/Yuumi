use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 对外上传 payload（Smart Split：当前玩家进 matchInfo，其余进外层 participants）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPayload {
    pub match_info: MatchInfo,
    pub participants: Vec<ParticipantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfo {
    pub match_id: u64,
    pub game_mode: String,
    pub game_type: String,
    pub queue_id: i32,
    pub game_creation: String,
    pub game_duration: u64,
    pub game_version: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub participants: Vec<ParticipantInfo>,
}

/// 上传的 participant 字段严格对齐 Python build_upload_payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantInfo {
    pub summoner_name: String,
    pub puuid: String,
    pub team_id: i32,
    pub champion_id: i32,
    pub champion_name: String,
    // Python: SummonerSpell1Id / SummonerSpell2Id（首字母大写）
    #[serde(rename = "SummonerSpell1Id")]
    pub summoner_spell1_id: i32,
    #[serde(rename = "SummonerSpell2Id")]
    pub summoner_spell2_id: i32,
    // Python: HexTech0 ~ HexTech4（首字母大写）
    #[serde(rename = "HexTech0")]
    pub hextech0: i32,
    #[serde(rename = "HexTech1")]
    pub hextech1: i32,
    #[serde(rename = "HexTech2")]
    pub hextech2: i32,
    #[serde(rename = "HexTech3")]
    pub hextech3: i32,
    #[serde(rename = "HexTech4")]
    pub hextech4: i32,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    // 伤害数据（对英雄）
    pub total_damage_dealt_to_champions: i32,
    pub physical_damage_dealt_to_champions: i32,
    pub magic_damage_dealt_to_champions: i32,
    pub true_damage_dealt_to_champions: i32,
    // 伤害数据（所有目标）
    pub total_damage_dealt: i32,
    pub physical_damage_dealt: i32,
    pub magic_damage_dealt: i32,
    pub true_damage_dealt: i32,
    // 承受伤害
    pub total_damage_taken: i32,
    pub physical_damage_taken: i32,
    pub magical_damage_taken: i32,
    // 治疗
    pub total_heal: i32,
    // 经济与补刀
    pub gold_earned: i32,
    pub gold_spent: i32,
    pub total_minions_killed: i32,
    pub neutral_minions_killed: i32,
    // 装备
    pub item0: i32,
    pub item1: i32,
    pub item2: i32,
    pub item3: i32,
    pub item4: i32,
    pub item5: i32,
    pub item6: i32,
    pub role_bound_item: i32,
    // 视野
    pub vision_score: i32,
    pub wards_placed: i32,
    pub wards_killed: i32,
    pub vision_wards_bought_in_game: i32,
    // 英雄等级
    pub champ_level: i32,
    // 多杀
    pub double_kills: i32,
    pub triple_kills: i32,
    pub quadra_kills: i32,
    pub penta_kills: i32,
    pub largest_multi_kill: i32,
    // 连杀
    pub largest_killing_spree: i32,
    pub killing_sprees: i32,
    // 全场最多标识
    pub most_kills: bool,
    pub most_assists: bool,
    pub most_damage_dealt: bool,
    pub most_damage_taken: bool,
    pub most_gold_earned: bool,
    pub most_turret_kills: bool,
    pub most_healing_done: bool,
    // 目标
    pub turret_kills: i32,
    pub inhibitor_kills: i32,
    pub first_blood_kill: bool,
    pub first_blood_assist: bool,
    pub first_tower_kill: bool,
    pub first_tower_assist: bool,
    // 伤害（对塔/目标）
    pub damage_dealt_to_turrets: i32,
    pub damage_dealt_to_objectives: i32,
    // 符文
    pub perk_primary_style: i32,
    pub perk_sub_style: i32,
}

// ─── LCU 原始数据结构 ───

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameDetail {
    pub game_id: Option<u64>,
    pub game_mode: Option<String>,
    pub game_type: Option<String>,
    pub queue_id: Option<i32>,
    pub game_creation: Option<u64>,
    pub game_duration: Option<u64>,
    pub game_version: Option<String>,
    #[serde(default)]
    pub participants: Vec<RawParticipant>,
    #[serde(default)]
    pub participant_identities: Vec<ParticipantIdentity>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawParticipant {
    pub participant_id: Option<i32>,
    pub team_id: Option<i32>,
    pub champion_id: Option<i32>,
    pub spell1_id: Option<i32>,
    pub spell2_id: Option<i32>,
    #[serde(default)]
    pub stats: ParticipantStats,
    // participant 层的海克斯强化数据（对齐 Python extract_hextech_ids 从 participant 读取）
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

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParticipantStats {
    #[serde(default)]
    pub win: bool,
    #[serde(default)]
    pub kills: i32,
    #[serde(default)]
    pub deaths: i32,
    #[serde(default)]
    pub assists: i32,
    #[serde(default)]
    pub total_damage_dealt_to_champions: i32,
    #[serde(default)]
    pub physical_damage_dealt_to_champions: i32,
    #[serde(default)]
    pub magic_damage_dealt_to_champions: i32,
    #[serde(default)]
    pub true_damage_dealt_to_champions: i32,
    #[serde(default)]
    pub total_damage_dealt: i32,
    #[serde(default)]
    pub physical_damage_dealt: i32,
    #[serde(default)]
    pub magic_damage_dealt: i32,
    #[serde(default)]
    pub true_damage_dealt: i32,
    #[serde(default)]
    pub total_damage_taken: i32,
    #[serde(default)]
    pub physical_damage_taken: i32,
    #[serde(default)]
    pub magical_damage_taken: i32,
    #[serde(default)]
    pub total_heal: i32,
    #[serde(default)]
    pub gold_earned: i32,
    #[serde(default)]
    pub gold_spent: i32,
    #[serde(default)]
    pub total_minions_killed: i32,
    #[serde(default)]
    pub neutral_minions_killed: i32,
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
    pub role_bound_item: i32,
    #[serde(default)]
    pub vision_score: i32,
    #[serde(default)]
    pub wards_placed: i32,
    #[serde(default)]
    pub wards_killed: i32,
    #[serde(default)]
    pub vision_wards_bought_in_game: i32,
    #[serde(default)]
    pub champ_level: i32,
    #[serde(default)]
    pub double_kills: i32,
    #[serde(default)]
    pub triple_kills: i32,
    #[serde(default)]
    pub quadra_kills: i32,
    #[serde(default)]
    pub penta_kills: i32,
    #[serde(default)]
    pub largest_multi_kill: i32,
    #[serde(default)]
    pub largest_killing_spree: i32,
    #[serde(default)]
    pub killing_sprees: i32,
    #[serde(default)]
    pub most_kills: bool,
    #[serde(default)]
    pub most_assists: bool,
    #[serde(default)]
    pub most_damage_dealt: bool,
    #[serde(default)]
    pub most_damage_taken: bool,
    #[serde(default)]
    pub most_gold_earned: bool,
    #[serde(default)]
    pub most_turret_kills: bool,
    #[serde(default)]
    pub most_healing_done: bool,
    #[serde(default)]
    pub turret_kills: i32,
    #[serde(default)]
    pub inhibitor_kills: i32,
    #[serde(default)]
    pub first_blood_kill: bool,
    #[serde(default)]
    pub first_blood_assist: bool,
    #[serde(default)]
    pub first_tower_kill: bool,
    #[serde(default)]
    pub first_tower_assist: bool,
    #[serde(default)]
    pub damage_dealt_to_turrets: i32,
    #[serde(default)]
    pub damage_dealt_to_objectives: i32,
    #[serde(default)]
    pub perk_primary_style: i32,
    #[serde(default)]
    pub perk_sub_style: i32,
    // 海克斯强化（用于 extract_hextech_ids）
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

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParticipantIdentity {
    pub participant_id: Option<i32>,
    pub player: Option<PlayerInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerInfo {
    #[serde(default)]
    pub puuid: String,
    pub game_name: Option<String>,
    pub summoner_name: Option<String>,
    pub tag_line: Option<String>,
}

/// 构建 Smart Split Payload：当前玩家进 matchInfo.participants，其余进外层 participants。
pub(crate) fn build_upload_payload(
    game_detail: &GameDetail,
    current_summoner_puuid: Option<&str>,
    champion_names: &HashMap<i32, String>,
) -> UploadPayload {
    let game_creation_iso = game_detail
        .game_creation
        .and_then(|ms| {
            let secs = (ms / 1000) as i64;
            chrono::DateTime::from_timestamp(secs, 0).map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string()
            })
        })
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());

    let match_info = MatchInfo {
        match_id: game_detail.game_id.unwrap_or(0),
        game_mode: game_detail.game_mode.clone().unwrap_or_default(),
        game_type: game_detail.game_type.clone().unwrap_or_default(),
        queue_id: game_detail.queue_id.unwrap_or(0),
        game_creation: game_creation_iso,
        game_duration: game_detail.game_duration.unwrap_or(0),
        game_version: game_detail.game_version.clone().unwrap_or_default(),
        participants: Vec::new(),
    };

    let pid_to_player: HashMap<i32, &PlayerInfo> = game_detail
        .participant_identities
        .iter()
        .filter_map(|ident| {
            let pid = ident.participant_id?;
            let player = ident.player.as_ref()?;
            Some((pid, player))
        })
        .collect();

    let mut all_participants: Vec<ParticipantInfo> = game_detail
        .participants
        .iter()
        .map(|p| {
            let pid = p.participant_id.unwrap_or(0);
            let player = pid_to_player.get(&pid);
            let summoner_name = player
                .map(|pl| {
                    let name = pl
                        .game_name
                        .as_deref()
                        .or(pl.summoner_name.as_deref())
                        .unwrap_or("Unknown");
                    match &pl.tag_line {
                        Some(tag) => format!("{}#{}", name, tag),
                        None => name.to_string(),
                    }
                })
                .unwrap_or_else(|| "Unknown".to_string());

            let puuid = player.map(|pl| pl.puuid.clone()).unwrap_or_default();
            let hextech = extract_hextech_ids(&p.stats, p);

            ParticipantInfo {
                summoner_name,
                puuid,
                team_id: p.team_id.unwrap_or(0),
                champion_id: p.champion_id.unwrap_or(0),
                champion_name: champion_names
                    .get(&p.champion_id.unwrap_or(0))
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
                summoner_spell1_id: p.spell1_id.unwrap_or(0),
                summoner_spell2_id: p.spell2_id.unwrap_or(0),
                hextech0: hextech[0],
                hextech1: hextech[1],
                hextech2: hextech[2],
                hextech3: hextech[3],
                hextech4: hextech[4],
                win: p.stats.win,
                kills: p.stats.kills,
                deaths: p.stats.deaths,
                assists: p.stats.assists,
                total_damage_dealt_to_champions: p.stats.total_damage_dealt_to_champions,
                physical_damage_dealt_to_champions: p.stats.physical_damage_dealt_to_champions,
                magic_damage_dealt_to_champions: p.stats.magic_damage_dealt_to_champions,
                true_damage_dealt_to_champions: p.stats.true_damage_dealt_to_champions,
                total_damage_dealt: p.stats.total_damage_dealt,
                physical_damage_dealt: p.stats.physical_damage_dealt,
                magic_damage_dealt: p.stats.magic_damage_dealt,
                true_damage_dealt: p.stats.true_damage_dealt,
                total_damage_taken: p.stats.total_damage_taken,
                physical_damage_taken: p.stats.physical_damage_taken,
                magical_damage_taken: p.stats.magical_damage_taken,
                total_heal: p.stats.total_heal,
                gold_earned: p.stats.gold_earned,
                gold_spent: p.stats.gold_spent,
                total_minions_killed: p.stats.total_minions_killed,
                neutral_minions_killed: p.stats.neutral_minions_killed,
                item0: p.stats.item0,
                item1: p.stats.item1,
                item2: p.stats.item2,
                item3: p.stats.item3,
                item4: p.stats.item4,
                item5: p.stats.item5,
                item6: p.stats.item6,
                role_bound_item: p.stats.role_bound_item,
                vision_score: p.stats.vision_score,
                wards_placed: p.stats.wards_placed,
                wards_killed: p.stats.wards_killed,
                vision_wards_bought_in_game: p.stats.vision_wards_bought_in_game,
                champ_level: p.stats.champ_level,
                double_kills: p.stats.double_kills,
                triple_kills: p.stats.triple_kills,
                quadra_kills: p.stats.quadra_kills,
                penta_kills: p.stats.penta_kills,
                largest_multi_kill: p.stats.largest_multi_kill,
                largest_killing_spree: p.stats.largest_killing_spree,
                killing_sprees: p.stats.killing_sprees,
                most_kills: p.stats.most_kills,
                most_assists: p.stats.most_assists,
                most_damage_dealt: p.stats.most_damage_dealt,
                most_damage_taken: p.stats.most_damage_taken,
                most_gold_earned: p.stats.most_gold_earned,
                most_turret_kills: p.stats.most_turret_kills,
                most_healing_done: p.stats.most_healing_done,
                turret_kills: p.stats.turret_kills,
                inhibitor_kills: p.stats.inhibitor_kills,
                first_blood_kill: p.stats.first_blood_kill,
                first_blood_assist: p.stats.first_blood_assist,
                first_tower_kill: p.stats.first_tower_kill,
                first_tower_assist: p.stats.first_tower_assist,
                damage_dealt_to_turrets: p.stats.damage_dealt_to_turrets,
                damage_dealt_to_objectives: p.stats.damage_dealt_to_objectives,
                perk_primary_style: p.stats.perk_primary_style,
                perk_sub_style: p.stats.perk_sub_style,
            }
        })
        .collect();

    // Smart Split
    let mut inner_participants = Vec::new();
    let mut outer_participants = Vec::new();

    if let Some(target_puuid) = current_summoner_puuid {
        let mut found = false;

        for p in all_participants.drain(..) {
            if !found && p.puuid == target_puuid {
                inner_participants.push(p);
                found = true;
            } else {
                outer_participants.push(p);
            }
        }

        if inner_participants.is_empty() && !outer_participants.is_empty() {
            inner_participants.push(outer_participants.remove(0));
        }
    } else if !all_participants.is_empty() {
        inner_participants.push(all_participants.remove(0));
        outer_participants = all_participants;
    }

    let mut match_info = match_info;
    match_info.participants = inner_participants;

    UploadPayload {
        match_info,
        participants: outer_participants,
    }
}

/// 对齐 Python _extract_augment_ids + extract_hextech_ids。
/// 同时从 stats 层和 participant 层提取 augments + playerAugment1~5，去重后返回 5 个值。
fn extract_hextech_ids(stats: &ParticipantStats, participant: &RawParticipant) -> [i32; 5] {
    let mut seen = HashSet::new();
    let mut ordered: Vec<i32> = Vec::new();

    // 对齐 Python: for source in (stats, participant)
    // 来源 1: stats 层
    for &id in &stats.augments {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    for id in [
        stats.player_augment1,
        stats.player_augment2,
        stats.player_augment3,
        stats.player_augment4,
        stats.player_augment5,
    ] {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }

    // 来源 2: participant 层（对齐 Python 从 participant 读取）
    for &id in &participant.augments {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    for id in [
        participant.player_augment1,
        participant.player_augment2,
        participant.player_augment3,
        participant.player_augment4,
        participant.player_augment5,
    ] {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }

    let mut result = [0i32; 5];
    for (i, &val) in ordered.iter().take(5).enumerate() {
        result[i] = val;
    }
    result
}
