use std::collections::HashMap;
use std::sync::Arc;

use super::augments::{extract_augments_from_value, get_tft_augments_cache_path};
use super::{
    convert_lcu_icon_path, convert_lcu_icon_path_to_cdragon, TftDataMapping, CACHED_AUGMENTS,
    CACHED_META_MAPS, CACHED_TFT_DATA,
};

/// CDragon TFT zh_cn.json 唯一下载入口（合并原 fetch_tft_data_mapping 与 fetch_tft_augments_raw
/// 各自独立的下载路径；proxy 为空则不走代理）
pub(crate) async fn fetch_cdragon_zh_cn(proxy: Option<&str>) -> Option<serde_json::Value> {
    let cdn_url = "https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json";
    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(60));

    if let Some(proxy_addr) = proxy {
        let formatted_proxy = if !proxy_addr.starts_with("http://")
            && !proxy_addr.starts_with("https://")
            && !proxy_addr.starts_with("socks5://")
        {
            format!("http://{}", proxy_addr)
        } else {
            proxy_addr.to_string()
        };
        if let Ok(p) = reqwest::Proxy::all(&formatted_proxy) {
            log::info!("[TFT CDN] CDragon 请求已成功配置代理: {}", formatted_proxy);
            builder = builder.proxy(p);
        } else {
            log::warn!("[TFT CDN] 代理地址解析失败: {}", formatted_proxy);
        }
    }

    let cdn_client = builder.build().ok()?;
    let resp = cdn_client.get(cdn_url).send().await.ok()?;
    let status = resp.status();
    if !status.is_success() {
        log::warn!("[TFT CDN] HTTP {}", status);
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
}

/// 从 TFT JSON 内容解析资源映射
fn parse_tft_data_from_value(root: &serde_json::Value) -> TftDataMapping {
    let mut mapping = TftDataMapping {
        champions: HashMap::new(),
        traits: HashMap::new(),
        champion_icons: HashMap::new(),
        trait_icons: HashMap::new(),
        item_icons: HashMap::new(),
        item_names: HashMap::new(),
    };

    let mut process_set_obj = |set_obj: &serde_json::Value| {
        if let Some(champs) = set_obj.get("champions").and_then(|v| v.as_array()) {
            for c in champs {
                if let Some(api_name) = c.get("apiName").and_then(|v| v.as_str()) {
                    let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let icon = c
                        .get("squareIcon")
                        .or_else(|| c.get("icon"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let api_lower = api_name.to_lowercase();
                    mapping
                        .champions
                        .insert(api_name.to_string(), name.to_string());
                    mapping
                        .champions
                        .insert(api_lower.clone(), name.to_string());

                    if !icon.is_empty() {
                        let icon_converted = convert_lcu_icon_path(icon);
                        mapping
                            .champion_icons
                            .insert(api_name.to_string(), icon_converted.clone());
                        mapping.champion_icons.insert(api_lower, icon_converted);
                    }
                }
            }
        }
        if let Some(traits) = set_obj.get("traits").and_then(|v| v.as_array()) {
            for t in traits {
                if let Some(api_name) = t.get("apiName").and_then(|v| v.as_str()) {
                    let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let icon = t.get("icon").and_then(|v| v.as_str()).unwrap_or("");
                    let api_lower = api_name.to_lowercase();
                    mapping
                        .traits
                        .insert(api_name.to_string(), name.to_string());
                    mapping.traits.insert(api_lower.clone(), name.to_string());

                    if !icon.is_empty() {
                        let icon_converted = convert_lcu_icon_path(icon);
                        mapping
                            .trait_icons
                            .insert(api_name.to_string(), icon_converted.clone());
                        mapping.trait_icons.insert(api_lower, icon_converted);
                    }
                }
            }
        }
    };

    if let Some(set_data) = root.get("setData").and_then(|v| v.as_array()) {
        for set_obj in set_data {
            process_set_obj(set_obj);
        }
    }

    if let Some(sets) = root.get("sets").and_then(|v| v.as_object()) {
        for (_k, set_obj) in sets {
            process_set_obj(set_obj);
        }
    }

    let mut process_item = |item: &serde_json::Value| {
        if let Some(api_name) = item.get("apiName").and_then(|v| v.as_str()) {
            let api_lower = api_name.to_lowercase();
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                mapping
                    .item_names
                    .insert(api_name.to_string(), name.to_string());
                mapping
                    .item_names
                    .insert(api_lower.clone(), name.to_string());
            }
            if let Some(icon) = item.get("icon").and_then(|v| v.as_str()) {
                let icon_converted = convert_lcu_icon_path(icon);
                mapping
                    .item_icons
                    .insert(api_name.to_string(), icon_converted.clone());
                mapping.item_icons.insert(api_lower, icon_converted);
            }
        }
    };

    if let Some(items) = root.get("items").and_then(|v| v.as_array()) {
        for item in items {
            process_item(item);
        }
    }

    if let Some(augments) = root.get("augments").and_then(|v| v.as_array()) {
        for aug in augments {
            process_item(aug);
        }
    }

    mapping
}

fn get_tft_data_cache_path() -> Option<std::path::PathBuf> {
    let dir = crate::runtime::app_data_dir().join("cache");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("tft_data.json"))
}

/// 抓取 TFT 基础数据字典（优先 LCU，备用 CDragon，带内存与磁盘缓存）
/// LCU → CDragon（一次，无重试，无代理）
pub(crate) async fn fetch_tft_data_mapping(_lcu: Option<&crate::LcuClient>) -> TftDataMapping {
    // 1. 内存缓存快路径（仅读锁，命中即返回）
    {
        let cache = CACHED_TFT_DATA.read().await;
        if let Some(ref mapping) = *cache {
            if !mapping.champions.is_empty() {
                return mapping.clone();
            }
        }
    }

    // 2. 尝试从本地磁盘缓存加载（无锁）
    if let Some(cache_path) = get_tft_data_cache_path() {
        if cache_path.exists() {
            if let Ok(file_content) = std::fs::read_to_string(&cache_path) {
                if let Ok(m) = serde_json::from_str::<TftDataMapping>(&file_content) {
                    if !m.champions.is_empty() {
                        let mut cache = CACHED_TFT_DATA.write().await;
                        *cache = Some(m.clone());
                        *CACHED_META_MAPS.write().await = None;
                        return m;
                    }
                }
            }
        }
    }

    // 3. CDragon CDN 一次请求不重试（合并下载入口，无锁执行，带超时防挂起）
    if let Some(root_val) = fetch_cdragon_zh_cn(None).await {
        let m = parse_tft_data_from_value(&root_val);
        if !m.champions.is_empty() {
            let m_for_cache = m.clone();
            let mut cache = CACHED_TFT_DATA.write().await;
            *cache = Some(m_for_cache);
            *CACHED_META_MAPS.write().await = None;
            let m_clone = m.clone();
            let aug_list = extract_augments_from_value(&root_val);
            log::info!("从 CDragon 成功提取到 {} 个 TFT 海克斯强化", aug_list.len());
            if !aug_list.is_empty() {
                let mut aug_cache = CACHED_AUGMENTS.write().await;
                *aug_cache = Some(aug_list.clone());
                tokio::spawn(async move {
                    if let Some(aug_path) = get_tft_augments_cache_path() {
                        if let Ok(json_str) = serde_json::to_string(&aug_list) {
                            let _ = std::fs::write(aug_path, json_str);
                        }
                    }
                });
            }
            tokio::spawn(async move {
                if let Some(cache_path) = get_tft_data_cache_path() {
                    if let Ok(json_str) = serde_json::to_string(&m_clone) {
                        let _ = std::fs::write(cache_path, json_str);
                    }
                }
            });
            return m;
        }
    }

    TftDataMapping {
        champions: HashMap::new(),
        traits: HashMap::new(),
        champion_icons: HashMap::new(),
        trait_icons: HashMap::new(),
        item_icons: HashMap::new(),
        item_names: HashMap::new(),
    }
}

/// 构建归一化的 trait name 映射（从 TftDataMapping 提取，不含网络请求）
fn build_trait_name_map_from_mapping(mapping: &TftDataMapping) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (api_name, display_name) in &mapping.traits {
        let parts: Vec<&str> = api_name.split('_').collect();
        let key = if parts.last().copied() == Some("Trait") && parts.len() > 1 {
            parts[parts.len() - 2]
        } else {
            parts.last().copied().unwrap_or(api_name.as_str())
        };
        let key = if key.ends_with("Trait") && key.len() > 5 {
            &key[..key.len() - 5]
        } else {
            key
        };
        let key_lower = key.to_lowercase();
        if !key_lower.is_empty() {
            result
                .entry(key_lower.clone())
                .or_insert_with(|| display_name.clone());
            result
                .entry(format!("{}trait", key_lower))
                .or_insert_with(|| display_name.clone());
        }
    }
    result
}

/// 尝试获取 TFT 数据映射填充缓存（LCU 优先，CDragon 兜底）
async fn ensure_tft_data_mapping(lcu: Option<&crate::LcuClient>) {
    fetch_tft_data_mapping(lcu).await;
}

/// 从内存缓存或 LCU/CDragon 获取 TFT 羁绊中文名称映射
pub async fn fetch_tft_meta_maps(lcu: Option<&crate::LcuClient>) -> serde_json::Value {
    ensure_tft_data_mapping(lcu).await;

    // 已构建过的映射直接复用 Value 缓存，避免缓存命中时仍逐次重建五个映射
    {
        let cached = CACHED_META_MAPS.read().await;
        if let Some(v) = cached.as_ref() {
            return (**v).clone();
        }
    }

    let cached = CACHED_TFT_DATA.read().await;
    let (trait_map, champ_icon_map, champ_name_map, item_name_map, item_icon_map) = match cached
        .as_ref()
    {
        Some(m) if !m.traits.is_empty() => {
            log::info!("TFT meta maps: 从缓存构建 ({} traits, {} champIcons, {} champNames, {} items, {} itemIcons)",
                m.traits.len(), m.champion_icons.len(), m.champions.len(), m.item_names.len(), m.item_icons.len());
            // 从 champion_icons 的 key 构建名称映射作为兜底
            let name_map = if m.champions.is_empty() {
                log::warn!("TFT meta maps: champion_name_map 为空，从 icon 键构建");
                let mut fallback = HashMap::new();
                for api_name in m.champion_icons.keys() {
                    let display = api_name.split('_').next_back().unwrap_or(api_name);
                    fallback.insert(api_name.clone(), display.to_string());
                    fallback.insert(api_name.to_lowercase(), display.to_string());
                }
                fallback
            } else {
                m.champions.clone()
            };
            // 将 champion_icons 路径转为 CDragon 直链（去掉 LCU 依赖）
            let cdn_icon_map: HashMap<String, String> = m
                .champion_icons
                .iter()
                .map(|(k, v)| {
                    let cdn = convert_lcu_icon_path_to_cdragon(v);
                    (k.clone(), cdn)
                })
                .collect();
            // 将 item_icons 路径转为 CDragon 直链
            let cdn_item_icon_map: HashMap<String, String> = m
                .item_icons
                .iter()
                .map(|(k, v)| {
                    let cdn = convert_lcu_icon_path_to_cdragon(v);
                    (k.clone(), cdn)
                })
                .collect();
            (
                build_trait_name_map_from_mapping(m),
                cdn_icon_map,
                name_map,
                m.item_names.clone(),
                cdn_item_icon_map,
            )
        }
        _ => {
            log::warn!("TFT meta maps: 无可用数据，返回空映射");
            (
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            )
        }
    };
    drop(cached);

    let mut result = serde_json::Map::new();
    result.insert(
        "trait_name_map".into(),
        serde_json::to_value(&trait_map).unwrap_or_default(),
    );
    result.insert(
        "champion_icon_map".into(),
        serde_json::to_value(&champ_icon_map).unwrap_or_default(),
    );
    result.insert(
        "champion_name_map".into(),
        serde_json::to_value(&champ_name_map).unwrap_or_default(),
    );
    result.insert(
        "item_name_map".into(),
        serde_json::to_value(&item_name_map).unwrap_or_default(),
    );
    result.insert(
        "item_icon_map".into(),
        serde_json::to_value(&item_icon_map).unwrap_or_default(),
    );
    let result = serde_json::Value::Object(result);

    // 非空数据才写入 Value 缓存（空映射无缓存价值，且避免覆盖为失效态）
    if !trait_map.is_empty() {
        *CACHED_META_MAPS.write().await = Some(Arc::new(result.clone()));
    }
    result
}
