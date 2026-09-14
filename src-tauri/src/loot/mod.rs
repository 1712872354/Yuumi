use serde::{Deserialize, Serialize};

pub mod actions;
pub mod inventory;
pub mod open;

pub use actions::{disenchant_loot, reroll_loot, upgrade_loot};
pub use inventory::{get_essence_balances, get_loot_inventory};
pub use open::{batch_open_loots, get_openable_loots, smart_open_all_loots};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenableLoot {
    pub loot_id: String,
    pub name: String,
    pub count: i32,
    pub recipe_name: String,
    pub need_key: bool,
    pub key_loot_id: Option<String>,
    pub key_count: i32,
    pub key_name: Option<String>,
    pub tile_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LootProgressEvent {
    pub current: i32,
    pub total: i32,
    pub success: bool,
    pub reward_name: String,
    pub error_msg: Option<String>,
    pub item_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenBatchItem {
    pub loot_id: String,
    pub name: String,
    pub count: i32,
    pub recipe_name: String,
    pub ingredients: Vec<String>,
}

/// 碎片库存条目（英雄/皮肤/表情/守卫皮肤/召唤师图标）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LootItem {
    pub loot_id: String,
    pub loot_name: String,
    pub item_desc: String,
    pub display_categories: String,
    pub loot_type: String,
    pub rarity: String,
    pub count: i32,
    pub value: i32,
    pub disenchant_value: i32,
    pub item_status: String,
    pub tile_path: Option<String>,
    pub upgrade_recipe_name: String,
    pub upgrade_essence_cost: i32,
    pub parent_item_status: String,
}

/// 批量分解请求条目
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisenchantItem {
    pub loot_id: String,
    pub count: i32,
    pub upgrade_recipe_name: Option<String>,
}

/// 碎片操作进度广播事件（分解/重随复用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionProgressEvent {
    pub current: i32,
    pub total: i32,
    pub success: bool,
    pub loot_name: String,
    pub reward_desc: String,
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EssenceBalances {
    pub blue_essence: i32,
    pub orange_essence: i32,
}

/// 计算战利品开启优先级（数字越小优先级越高）
pub(crate) fn loot_priority(loot_id: &str) -> i32 {
    // 优先级：宝箱 > 海克斯宝箱 > 法球/胶囊 > 其他
    if loot_id.contains("CHEST") && !loot_id.contains("hextech") && !loot_id.contains("premium") {
        0 // 普通宝箱 — 最高优先级
    } else if loot_id == "CHEST_hextech" || loot_id.contains("hextech") {
        1 // 海克斯宝箱
    } else if loot_id.contains("ORB") || loot_id.contains("orb") {
        2 // 法球
    } else if loot_id.contains("CAPSULE") || loot_id.contains("capsule") {
        3 // 胶囊
    } else {
        4 // 其他
    }
}

fn is_key_fragment_loot_id(loot_id: &str) -> bool {
    loot_id.eq_ignore_ascii_case("MATERIAL_key_fragment")
}

pub(crate) fn is_openable_loot(loot_type: &str, loot_id: &str) -> bool {
    let loot_type_upper = loot_type.to_ascii_uppercase();
    let loot_id_upper = loot_id.to_ascii_uppercase();

    matches!(loot_type_upper.as_str(), "CHEST" | "ORB" | "PORTAL")
        || loot_id_upper.contains("ORB")
        || loot_id_upper.contains("CHEST")
        || (loot_type_upper == "MATERIAL"
            && (loot_id_upper.contains("ORB")
                || loot_id_upper.contains("CHEST")
                || loot_id_upper.contains("CAPSULE")
                || is_key_fragment_loot_id(loot_id)))
}

async fn fetch_recipes(
    base: &str,
    auth: &str,
    http_client: &reqwest::Client,
    loot_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let recipe_url = format!("{}/lol-loot/v1/recipes/initial-item/{}", base, loot_id);
    let resp = http_client
        .get(&recipe_url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取配方接口返回错误: {}", resp.status()));
    }

    resp.json().await.map_err(|e| e.to_string())
}

fn find_recipe_by_name<'a>(
    recipes: &'a [serde_json::Value],
    recipe_name: &str,
) -> Option<&'a serde_json::Value> {
    recipes.iter().find(|r| {
        r.get("recipeName")
            .and_then(|v| v.as_str())
            .map(|name| name == recipe_name)
            .unwrap_or(false)
    })
}

fn find_recipe_by_keywords<'a>(
    recipes: &'a [serde_json::Value],
    keywords: &[&str],
) -> Option<&'a str> {
    for keyword in keywords {
        let keyword_lower = keyword.to_ascii_lowercase();
        if let Some(recipe_name) = recipes.iter().find_map(|r| {
            let recipe_name = r.get("recipeName").and_then(|v| v.as_str())?;
            recipe_name
                .to_ascii_lowercase()
                .contains(&keyword_lower)
                .then_some(recipe_name)
        }) {
            return Some(recipe_name);
        }
    }
    None
}

fn recipe_name(recipe: &serde_json::Value) -> Result<&str, String> {
    recipe
        .get("recipeName")
        .and_then(|v| v.as_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "配方缺少 recipeName 字段".to_string())
}

fn collect_extra_ingredients(recipe: &serde_json::Value) -> Vec<String> {
    let mut extra_ingredients = Vec::new();
    collect_currency_ingredients(recipe, &mut extra_ingredients);
    extra_ingredients.sort();
    extra_ingredients.dedup();
    extra_ingredients
}

fn collect_currency_ingredients(value: &serde_json::Value, result: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if matches!(s.as_str(), "CURRENCY_champion" | "CURRENCY_cosmetic") {
                result.push(s.clone());
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_currency_ingredients(value, result);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_currency_ingredients(value, result);
            }
        }
        _ => {}
    }
}

/// 动态查询某碎片支持的特定动作配方名（按关键字匹配 recipeName）
/// 关键字如 "disenchant" / "reroll" / "forge"。避免硬编码配方名随版本失效。
pub(crate) async fn find_recipe_name(
    base: &str,
    auth: &str,
    http_client: &reqwest::Client,
    loot_id: &str,
    keyword: &str,
) -> Result<String, String> {
    let recipes = fetch_recipes(base, auth, http_client, loot_id).await?;
    if let Some(recipe) = find_recipe_by_keywords(&recipes, &[keyword]) {
        return Ok(recipe.to_string());
    }

    Err(format!("未找到包含关键字 '{}' 的配方", keyword))
}

/// 升级配方查找与额外材料检测：依次尝试 permanent → upgrade → open，并在匹配成功的配方中动态检测是否包含 CURRENCY_champion / CURRENCY_cosmetic 等额外材料
pub(crate) async fn find_recipe_upgrade_info(
    base: &str,
    auth: &str,
    http_client: &reqwest::Client,
    loot_id: &str,
    specific_recipe_name: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let recipes = fetch_recipes(base, auth, http_client, loot_id).await?;

    let mut selected_recipe: Option<&serde_json::Value> = None;
    if let Some(r_name) = specific_recipe_name {
        if !r_name.is_empty() {
            selected_recipe = find_recipe_by_name(&recipes, r_name);
        }
    }

    if selected_recipe.is_none() {
        if let Some(selected_recipe_name) = find_recipe_by_keywords(
            &recipes,
            &[
                "permanent",
                "upgrade",
                "open",
                "claim",
                "activate",
                "unlock",
            ],
        ) {
            selected_recipe = find_recipe_by_name(&recipes, selected_recipe_name);
        }
    }

    if let Some(r) = selected_recipe {
        let recipe_name = recipe_name(r)?.to_string();
        let extra_ingredients = collect_extra_ingredients(r);
        log::info!(
            "[loot][upgrade] {} 确定配方: {}, 额外材料: {:?}",
            loot_id,
            recipe_name,
            extra_ingredients
        );
        Ok((recipe_name, extra_ingredients))
    } else {
        let loot_upper = loot_id.to_uppercase();
        // 针对头像图标 (ICON)、表情 (EMOTE) 和守卫皮肤 (WARDSKIN) 等免费道具，如果配方列表未返回，尝试使用 LCU 静态定义的分类通用解锁配方
        if loot_upper.contains("ICON") {
            let guessed_recipe = "SUMMONERICON_open".to_string();
            log::warn!(
                "[loot][upgrade] {} 未在配方列表中找到，尝试使用通用图标解锁配方: {}",
                loot_id,
                guessed_recipe
            );
            return Ok((guessed_recipe, Vec::new()));
        } else if loot_upper.contains("EMOTE") {
            let guessed_recipe = "EMOTE_open".to_string();
            log::warn!(
                "[loot][upgrade] {} 未在配方列表中找到，尝试使用通用表情解锁配方: {}",
                loot_id,
                guessed_recipe
            );
            return Ok((guessed_recipe, Vec::new()));
        } else if loot_upper.contains("WARDSKIN") {
            let guessed_recipe = "WARDSKIN_open".to_string();
            log::warn!(
                "[loot][upgrade] {} 未在配方列表中找到，尝试使用通用守卫解锁配方: {}",
                loot_id,
                guessed_recipe
            );
            return Ok((guessed_recipe, Vec::new()));
        }

        let recipe_names: Vec<String> = recipes
            .iter()
            .filter_map(|r| {
                r.get("recipeName")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();
        log::warn!(
            "[loot][upgrade] {} 无解锁配方，可用: {:?}",
            loot_id,
            recipe_names
        );

        let extra_tip = if loot_upper.contains("ICON") || loot_upper.contains("EMOTE") {
            " (提示：头像/表情无法重复解锁。此处无解锁配方，通常代表您的账号已拥有该永久道具，您可以将其进行「分解」或「重随」)。"
        } else {
            ""
        };

        Err(format!(
            "未找到解锁/升级配方（已尝试 permanent/upgrade/open/claim/activate/unlock），可用配方: {:?}{}",
            recipe_names, extra_tip
        ))
    }
}
