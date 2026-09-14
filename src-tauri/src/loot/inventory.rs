use tauri::State;

use super::*;
use crate::{build_auth_header, AppState};

/// 4. 获取玩家所有碎片类战利品（排除材料/货币/箱子）
#[tauri::command]
pub async fn get_loot_inventory(app_state: State<'_, AppState>) -> Result<Vec<LootItem>, String> {
    // 尽早释放读锁，避免 HTTP 请求期间阻塞 monitor 重连
    let lcu = app_state.lcu_params().await?;
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client;

    let url = format!("{}/lol-loot/v1/player-loot", base);
    let resp = http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw_loots: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    // 建立各类已拥有道具的 Hash 集合，解决 LCU 战利品列表中 itemStatus 始终为 NONE 的官方 Bug
    let mut owned_skin_ids = std::collections::HashSet::new();
    let mut owned_icon_ids = std::collections::HashSet::new();
    let mut owned_emote_ids = std::collections::HashSet::new();
    let mut owned_ward_skin_ids = std::collections::HashSet::new();

    let summoner_url = format!("{}/lol-summoner/v1/current-summoner", base);
    if let Ok(summoner_resp) = http_client
        .get(&summoner_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        if summoner_resp.status().is_success() {
            if let Ok(summoner_json) = summoner_resp.json::<serde_json::Value>().await {
                if let Some(summoner_id) = summoner_json.get("summonerId").and_then(|v| v.as_i64())
                {
                    // 1-4. 并发拉取已拥有皮肤/头像/表情/守卫皮肤（原串行 4 个大请求，并发显著降低耗时）
                    let (skins, icons, emotes, wards) = tokio::join!(
                        fetch_owned_skins(&http_client, &base, &auth, summoner_id),
                        fetch_owned_icons(&http_client, &base, &auth, summoner_id),
                        fetch_owned_emotes(&http_client, &base, &auth),
                        fetch_owned_ward_skins(&http_client, &base, &auth, summoner_id),
                    );
                    owned_skin_ids = skins;
                    owned_icon_ids = icons;
                    owned_emote_ids = emotes;
                    owned_ward_skin_ids = wards;
                }
            }
        }
    }

    let mut result = Vec::new();

    for item in raw_loots {
        let display_category = item
            .get("displayCategories")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let loot_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let loot_id = item.get("lootId").and_then(|v| v.as_str()).unwrap_or("");
        let loot_id_trim = loot_id.trim();
        if loot_id_trim.is_empty() || loot_id_trim.to_uppercase().starts_with("TFT") {
            continue;
        }

        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        if count <= 0 {
            continue;
        }

        log::debug!(
            "[loot][inventory] lootId={} type={} displayCategories={} count={}",
            loot_id,
            loot_type,
            display_category,
            count
        );

        let mut item_desc = item
            .get("localizedName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                item.get("itemDesc")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or(loot_id)
            .to_string();

        if item_desc.trim().is_empty() {
            continue;
        }

        if item_desc == "MATERIAL_key_fragment" {
            item_desc = "钥匙碎片".to_string();
        } else if item_desc == "MATERIAL_key" {
            item_desc = "海克斯科技钥匙".to_string();
        } else if item_desc == "MATERIAL_key_premium" {
            item_desc = "杰作钥匙".to_string();
        } else if item_desc.eq_ignore_ascii_case("chest_128") {
            item_desc = "英雄魔法引擎".to_string();
        } else if item_desc.eq_ignore_ascii_case("chest_129") {
            item_desc = "荣耀英雄魔法引擎".to_string();
        }
        let value = item.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let disenchant_value = item
            .get("disenchantValue")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let store_item_id = item
            .get("storeItemId")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let mut item_status = item
            .get("itemStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("NONE")
            .to_string();

        let display_upper = display_category.to_uppercase();
        if store_item_id > 0 {
            let is_owned = (display_upper == "SKIN" && owned_skin_ids.contains(&store_item_id))
                || (display_upper == "SUMMONERICON" && owned_icon_ids.contains(&store_item_id))
                || (display_upper == "EMOTE" && owned_emote_ids.contains(&store_item_id))
                || (display_upper == "WARDSKIN" && owned_ward_skin_ids.contains(&store_item_id));
            if is_owned {
                item_status = "OWNED".to_string();
            }
        }
        let mut tile_path = item
            .get("tilePath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if tile_path.is_none() {
            tile_path = item
                .get("imagePath")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
        let rarity = item
            .get("rarity")
            .and_then(|v| v.as_str())
            .unwrap_or("DEFAULT")
            .to_string();
        let loot_name = item
            .get("lootName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let upgrade_recipe_name = item
            .get("upgradeRecipeName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let upgrade_essence_cost = item
            .get("upgradeEssenceValue")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let parent_item_status = item
            .get("parentItemStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        result.push(LootItem {
            loot_id: loot_id.to_string(),
            loot_name,
            item_desc,
            display_categories: display_category.to_string(),
            loot_type: loot_type.to_string(),
            rarity,
            count,
            value,
            disenchant_value,
            item_status,
            tile_path,
            upgrade_recipe_name,
            upgrade_essence_cost,
            parent_item_status,
        });
    }

    Ok(result)
}

/// 获取已拥有皮肤 ID 集合
async fn fetch_owned_skins(
    http_client: &reqwest::Client,
    base: &str,
    auth: &str,
    summoner_id: i64,
) -> std::collections::HashSet<i32> {
    let mut owned = std::collections::HashSet::new();
    let url = format!(
        "{}/lol-champions/v1/inventories/by-summoner/{}/skins",
        base, summoner_id
    );
    if let Ok(skins_resp) = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
    {
        if skins_resp.status().is_success() {
            if let Ok(skins_json) = skins_resp.json::<Vec<serde_json::Value>>().await {
                for s in skins_json {
                    if let (Some(skin_id), Some(ownership)) =
                        (s.get("id").and_then(|v| v.as_i64()), s.get("ownership"))
                    {
                        if ownership
                            .get("owned")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            owned.insert(skin_id as i32);
                        }
                    }
                }
            }
        }
    }
    owned
}

/// 获取已拥有头像图标 ID 集合
async fn fetch_owned_icons(
    http_client: &reqwest::Client,
    base: &str,
    auth: &str,
    summoner_id: i64,
) -> std::collections::HashSet<i32> {
    let mut owned = std::collections::HashSet::new();
    let url = format!(
        "{}/lol-collections/v1/inventories/{}/summoner-icons",
        base, summoner_id
    );
    if let Ok(icons_resp) = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
    {
        if icons_resp.status().is_success() {
            if let Ok(icons_json) = icons_resp.json::<serde_json::Value>().await {
                if let Some(arr) = icons_json.as_array() {
                    for item in arr {
                        if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
                            owned.insert(id as i32);
                        } else if let Some(id) = item.get("iconId").and_then(|v| v.as_i64()) {
                            owned.insert(id as i32);
                        }
                    }
                } else if let Some(obj) = icons_json.as_object() {
                    if let Some(icons_arr) = obj.get("icons").and_then(|v| v.as_array()) {
                        for item in icons_arr {
                            if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
                                owned.insert(id as i32);
                            } else if let Some(id) = item.get("iconId").and_then(|v| v.as_i64()) {
                                owned.insert(id as i32);
                            }
                        }
                    }
                }
            }
        }
    }
    owned
}

/// 获取已拥有表情 ID 集合
async fn fetch_owned_emotes(
    http_client: &reqwest::Client,
    base: &str,
    auth: &str,
) -> std::collections::HashSet<i32> {
    let mut owned = std::collections::HashSet::new();
    let url = format!("{}/lol-inventory/v1/inventory/emotes", base);
    if let Ok(emotes_resp) = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
    {
        if emotes_resp.status().is_success() {
            if let Ok(emotes_json) = emotes_resp.json::<serde_json::Value>().await {
                if let Some(arr) = emotes_json.as_array() {
                    for item in arr {
                        if let Some(id) = item.get("itemId").and_then(|v| v.as_i64()) {
                            owned.insert(id as i32);
                        } else if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
                            owned.insert(id as i32);
                        }
                    }
                }
            }
        }
    }
    owned
}

/// 获取已拥有守卫皮肤 ID 集合
async fn fetch_owned_ward_skins(
    http_client: &reqwest::Client,
    base: &str,
    auth: &str,
    summoner_id: i64,
) -> std::collections::HashSet<i32> {
    let mut owned = std::collections::HashSet::new();
    let url = format!(
        "{}/lol-collections/v1/inventories/{}/ward-skins",
        base, summoner_id
    );
    if let Ok(ward_skins_resp) = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
    {
        if ward_skins_resp.status().is_success() {
            if let Ok(ward_skins_json) = ward_skins_resp.json::<serde_json::Value>().await {
                if let Some(arr) = ward_skins_json.as_array() {
                    for item in arr {
                        if let (Some(ward_skin_id), Some(ownership)) = (
                            item.get("id").and_then(|v| v.as_i64()),
                            item.get("ownership"),
                        ) {
                            if ownership
                                .get("owned")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                owned.insert(ward_skin_id as i32);
                            }
                        }
                    }
                }
            }
        }
    }
    owned
}

/// 8. 获取玩家蓝/橙精粹余额
#[tauri::command]
pub async fn get_essence_balances(
    app_state: State<'_, AppState>,
) -> Result<EssenceBalances, String> {
    let lcu = app_state.lcu_params().await?;
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client;

    let url = format!("{}/lol-loot/v1/player-loot", base);
    let resp = http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw_loots: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    let mut blue_essence = 0;
    let mut orange_essence = 0;

    for item in raw_loots {
        if let Some(loot_id) = item.get("lootId").and_then(|v| v.as_str()) {
            if loot_id == "CURRENCY_champion" {
                blue_essence = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            } else if loot_id == "CURRENCY_cosmetic" {
                orange_essence = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            }
        }
    }

    Ok(EssenceBalances {
        blue_essence,
        orange_essence,
    })
}
