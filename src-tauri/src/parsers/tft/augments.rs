use tauri::State;

use crate::AppState;

use super::data::fetch_cdragon_zh_cn;
use super::{convert_lcu_icon_path, TftAugmentInfo, CACHED_AUGMENTS};

pub(crate) fn get_tft_augments_cache_path() -> Option<std::path::PathBuf> {
    let dir = crate::runtime::app_data_dir().join("cache");
    std::fs::create_dir_all(&dir).ok()?;
    // 缓存版本号 +1：旧版缓存含未解析的 @占位符@，需要强制重新生成
    let v2_path = dir.join("tft_augments_v2.json");
    let legacy_path = dir.join("tft_augments.json");
    if legacy_path.exists() {
        let _ = std::fs::remove_file(&legacy_path);
    }
    Some(v2_path)
}

fn clean_tft_desc(desc_raw: &str) -> String {
    if desc_raw.is_empty() {
        return String::new();
    }
    let mut result = String::with_capacity(desc_raw.len());
    let mut in_tag = false;
    for ch in desc_raw.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .trim()
        .to_string()
}

/// 将海克斯描述中的 @变量@ 占位符解析为实际数值。
/// 数据来源：CDragon 中每个强化对象自带的 effects 字段（变量名 → 数值），
/// 支持 `@Var*100@`、`@AttackSpeed*100*MaxStacks@` 这类算术表达式。
/// 无法静态解析的动态占位符（TFTTrait/TFTUnitProperty 等运行期值）直接移除。
fn resolve_tft_desc_tokens(
    desc: &str,
    effects: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let mut out = String::with_capacity(desc.len());
    let mut rest = desc;

    while let Some(start) = rest.find('@') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('@') {
            Some(inner) => {
                let token = &after[..inner];
                if let Some(value) = eval_tft_token(token, effects) {
                    out.push_str(&format_tft_number(value));
                }
                rest = &after[inner + 1..];
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }

    out.push_str(rest);
    out
}

/// 计算单个 @...@ 占位符表达式（形如 `VarName` 或 `VarName*100*MaxStacks`），
/// 变量名查找大小写不敏感。
fn eval_tft_token(
    token: &str,
    effects: &serde_json::Map<String, serde_json::Value>,
) -> Option<f64> {
    if token.contains(':') || token.contains("TFTTrait") || token.contains("TFTUnitProperty") {
        return None;
    }
    let mut acc = 1.0_f64;
    for factor in token.split('*') {
        let factor = factor.trim().trim_end_matches('%').trim();
        if factor.is_empty() {
            continue;
        }
        if let Ok(num) = factor.parse::<f64>() {
            acc *= num;
            continue;
        }
        let value = effects
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(factor))
            .and_then(|(_, v)| v.as_f64())?;
        acc *= value;
    }
    Some(acc)
}

/// 格式化数值：整数不带小数点，小数最多保留两位并去掉多余的 0
fn format_tft_number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.trunc()).abs() < 1e-9 {
        format!("{}", rounded as i64)
    } else {
        format!("{:.2}", rounded)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn extract_augment_tier(api_name: &str, icon: &str, name: &str) -> i32 {
    let lower_api = api_name.to_lowercase();
    let lower_icon = icon.to_lowercase();

    if lower_api.contains("prismatic")
        || lower_icon.contains("prismatic")
        || lower_api.contains("tier3")
        || lower_icon.contains("tier3")
        || lower_api.contains("hr_t3")
    {
        return 3;
    }
    if lower_api.contains("gold")
        || lower_icon.contains("gold")
        || lower_api.contains("tier2")
        || lower_icon.contains("tier2")
        || lower_api.contains("hr_t2")
    {
        return 2;
    }
    if lower_api.contains("silver")
        || lower_icon.contains("silver")
        || lower_api.contains("tier1")
        || lower_icon.contains("tier1")
        || lower_api.contains("hr_t1")
    {
        return 1;
    }

    if name.ends_with(" III") || name.contains(" III ") {
        return 3;
    }
    if name.ends_with(" II") || name.contains(" II ") {
        return 2;
    }
    if name.ends_with(" I") || name.contains(" I ") {
        return 1;
    }

    2
}

pub(crate) fn extract_augments_from_value(root: &serde_json::Value) -> Vec<TftAugmentInfo> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut process_item_val = |item: &serde_json::Value, force_augment: bool| {
        let name = item
            .get("name")
            .or_else(|| item.get("displayName"))
            .or_else(|| item.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() {
            return;
        }

        let api_name = item
            .get("apiName")
            .or_else(|| item.get("api_name"))
            .or_else(|| item.get("hexId"))
            .and_then(|v| v.as_str())
            .unwrap_or(name);
        if seen.contains(api_name) {
            return;
        }

        let icon = item
            .get("icon")
            .or_else(|| item.get("squareIcon"))
            .or_else(|| item.get("imgUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let lower_api = api_name.to_lowercase();
        let lower_icon = icon.to_lowercase();

        let is_aug = force_augment
            || lower_api.contains("augment")
            || lower_icon.contains("augment")
            || (lower_api.starts_with("tft")
                && (lower_api.contains("aug")
                    || lower_api.contains("superrune")
                    || lower_api.contains("hextech")));

        if !is_aug {
            return;
        }

        seen.insert(api_name.to_string());

        let raw_desc = item
            .get("desc")
            .or_else(|| item.get("description"))
            .or_else(|| item.get("descClean"))
            .or_else(|| item.get("tooltip"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let effects = item
            .get("effects")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let desc = resolve_tft_desc_tokens(&clean_tft_desc(raw_desc), &effects);
        let icon_path = convert_lcu_icon_path(icon);
        let tier = extract_augment_tier(api_name, icon, name);

        result.push(TftAugmentInfo {
            api_name: api_name.to_string(),
            name: name.to_string(),
            desc,
            icon_path,
            tier,
        });
    };

    let root_keys: Vec<&str> = root
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    // 1. 根节点自身为 Array（某些 LCU 端点的裸数组结构）
    if let Some(arr) = root.as_array() {
        log::info!("[Extract] 根节点为 Array，共 {} 个元素", arr.len());
        for item in arr {
            process_item_val(item, false);
        }
    }

    // 2. 根节点为 Object — 从各字段提取
    let has_augments_field = root_keys.contains(&"augments");
    let has_set_data = root_keys.contains(&"setData");
    let has_sets = root_keys.contains(&"sets");
    let has_items = root_keys.contains(&"items");
    log::info!(
        "[Extract] Object 字段: augments={}, setData={}, sets={}, items={}",
        has_augments_field,
        has_set_data,
        has_sets,
        has_items
    );

    if let Some(arr) = root.get("augments").and_then(|v| v.as_array()) {
        log::info!("[Extract] 根节点 augments[] = {} 项", arr.len());
        for item in arr {
            process_item_val(item, true);
        }
    }

    if let Some(arr) = root.get("items").and_then(|v| v.as_array()) {
        for item in arr {
            process_item_val(item, false);
        }
    }

    if let Some(set_data) = root.get("setData").and_then(|v| v.as_array()) {
        for set_obj in set_data {
            if let Some(items) = set_obj.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    process_item_val(item, false);
                }
            }
            if let Some(augments) = set_obj.get("augments").and_then(|v| v.as_array()) {
                for item in augments {
                    process_item_val(item, true);
                }
            }
        }
    }

    if let Some(sets) = root.get("sets").and_then(|v| v.as_object()) {
        for (_k, set_obj) in sets {
            if let Some(items) = set_obj.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    process_item_val(item, false);
                }
            }
            if let Some(augments) = set_obj.get("augments").and_then(|v| v.as_array()) {
                for item in augments {
                    process_item_val(item, true);
                }
            }
        }
    }

    result
}

async fn fetch_tft_augments_raw(app_state: &AppState) -> Vec<TftAugmentInfo> {
    log::info!("[TFT Augments Backend] 准备抓取海克斯强化 (CDragon CDN)");

    let proxy_addr = {
        let cfg = app_state.config.read().await;
        if cfg.general.enable_http_proxy {
            Some(cfg.general.http_proxy_addr.trim().to_string())
        } else {
            None
        }
    };

    log::info!("[TFT CDN] 尝试: https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json");

    match fetch_cdragon_zh_cn(proxy_addr.as_deref()).await {
        Some(root_val) => {
            let aug_list = extract_augments_from_value(&root_val);
            log::info!("[TFT CDN] 提取到 {} 个海克斯强化", aug_list.len());
            aug_list
        }
        None => {
            log::warn!("[TFT CDN] 请求失败或 JSON 解析失败");
            Vec::new()
        }
    }
}

/// 从已解析的 tft.json 缓存中提取所有海克斯强化信息（支持强制跳过缓存刷新）
#[tauri::command]
pub async fn get_tft_augments(
    force_refresh: Option<bool>,
    app_state: State<'_, AppState>,
) -> Result<Vec<TftAugmentInfo>, String> {
    let force = force_refresh.unwrap_or(false);
    log::info!(
        "[Tauri Cmd] >>> get_tft_augments 命令被触发！(force_refresh={})",
        force
    );

    if !force {
        // 1. 尝试读取内存缓存
        {
            let cache = CACHED_AUGMENTS.read().await;
            if let Some(ref list) = *cache {
                if !list.is_empty() {
                    return Ok(list.clone());
                }
            }
        }

        // 2. 尝试读取本地磁盘缓存
        if let Some(path) = get_tft_augments_cache_path() {
            if path.exists() {
                if let Ok(file_content) = std::fs::read_to_string(&path) {
                    if let Ok(list) = serde_json::from_str::<Vec<TftAugmentInfo>>(&file_content) {
                        if !list.is_empty() {
                            let mut cache = CACHED_AUGMENTS.write().await;
                            *cache = Some(list.clone());
                            return Ok(list);
                        } else {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    // 3. CDragon CDN 抓取（支持系统/用户代理）
    let list = fetch_tft_augments_raw(&app_state).await;

    if !list.is_empty() {
        let mut cache = CACHED_AUGMENTS.write().await;
        *cache = Some(list.clone());
        let list_for_cache = list.clone();
        tokio::spawn(async move {
            if let Some(aug_path) = get_tft_augments_cache_path() {
                if let Ok(json_str) = serde_json::to_string(&list_for_cache) {
                    let _ = std::fs::write(aug_path, json_str);
                }
            }
        });
        return Ok(list);
    }

    // 4. 若网络重新抓取失败或超时，安全回滚：优先使用现有的本地磁盘或内存缓存，避免清空界面数据
    log::warn!("TFT 海克斯网络抓取失败或超时，尝试安全回滚使用本地缓存");
    {
        let cache = CACHED_AUGMENTS.read().await;
        if let Some(ref old_list) = *cache {
            if !old_list.is_empty() {
                return Ok(old_list.clone());
            }
        }
    }
    if let Some(path) = get_tft_augments_cache_path() {
        if path.exists() {
            if let Ok(file_content) = std::fs::read_to_string(&path) {
                if let Ok(old_list) = serde_json::from_str::<Vec<TftAugmentInfo>>(&file_content) {
                    if !old_list.is_empty() {
                        return Ok(old_list);
                    }
                }
            }
        }
    }

    Ok(Vec::new())
}
