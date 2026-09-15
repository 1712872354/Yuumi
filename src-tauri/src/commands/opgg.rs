use tauri::State;

use crate::AppState;

/// 从 OP.GG API 获取英雄梯队/出装数据（代理请求，避免前端 CORS，缓存与客户端复用见 lcu::opgg）
#[tauri::command]
pub async fn fetch_opgg_data(
    region: String,
    mode: String,
    tier: String,
    champion_id: Option<i32>,
    position: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let cache_key = format!(
        "{}_{}_{}_{:?}_{:?}",
        region, mode, tier, champion_id, position
    );

    let url = match champion_id {
        Some(id) => {
            let pos = position.unwrap_or_else(|| "none".into());
            if mode == "arena" {
                format!(
                    "https://lol-api-champion.op.gg/api/{}/champions/{}",
                    region, id
                )
            } else {
                format!(
                    "https://lol-api-champion.op.gg/api/{}/champions/{}/{}/{}",
                    region, mode, id, pos
                )
            }
        }
        None => format!(
            "https://lol-api-champion.op.gg/api/{}/champions/{}",
            region, mode
        ),
    };

    crate::lcu::opgg::get_json(
        app_state.inner(),
        &url,
        &[("tier", tier.as_str())],
        &cache_key,
    )
    .await
}

/// 附加羁绊/英雄名称/图标映射到 MCP 阵容数据（fetch_tft_meta_maps 内部有 Value 形态缓存，命中时仅浅克隆）
async fn attach_tft_meta_maps(parsed: &mut serde_json::Value) {
    // 映射字典不依赖 LCU 客户端（走内存/磁盘/CDragon 缓存），无需持锁
    let meta_maps = crate::parsers::tft::fetch_tft_meta_maps(None).await;
    if let Some(obj) = parsed.as_object_mut() {
        if let Some(trait_map) = meta_maps.get("trait_name_map") {
            obj.insert("trait_name_map".to_string(), trait_map.clone());
        }
        if let Some(champ_icon_map) = meta_maps.get("champion_icon_map") {
            obj.insert("champion_icon_map".to_string(), champ_icon_map.clone());
        }
        if let Some(champ_name_map) = meta_maps.get("champion_name_map") {
            obj.insert("champion_name_map".to_string(), champ_name_map.clone());
        }
        if let Some(item_name_map) = meta_maps.get("item_name_map") {
            obj.insert("item_name_map".to_string(), item_name_map.clone());
        }
        if let Some(item_icon_map) = meta_maps.get("item_icon_map") {
            obj.insert("item_icon_map".to_string(), item_icon_map.clone());
        }
    }
}

/// 从 OP.GG MCP API 获取云顶之弈当前版本热门强势阵容
#[tauri::command]
pub async fn fetch_tft_meta_decks(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let cache_key = "tft_meta_decks".to_string();

    // 尝试内存/磁盘缓存（阵容本体与图标映射均有独立缓存）
    if let Some(mut parsed) = crate::lcu::opgg::get_cached(&cache_key).await {
        attach_tft_meta_maps(&mut parsed).await;
        return Ok(parsed);
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "tft_list_meta_decks",
            "arguments": {
                "desired_output_fields": [
                    "id",
                    "name",
                    "cost",
                    "teamCode",
                    "badge",
                    "stat",
                    "traits",
                    "units",
                    "early",
                    "middle"
                ]
            }
        }
    });

    let raw = crate::lcu::opgg::post_json(
        app_state.inner(),
        "https://mcp-api.op.gg/mcp",
        &request_body,
    )
    .await?;

    // MCP 错误检查
    if let Some(err) = raw.get("error") {
        let msg = err["message"].as_str().unwrap_or("未知 MCP 错误");
        return Err(format!("OP.GG MCP 返回错误: {}", msg));
    }

    // 提取 content[].text 中的 JSON 字符串
    let content_text = raw["result"]["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|first| first["text"].as_str())
        .ok_or_else(|| "OP.GG MCP 响应格式异常: 缺少 result.content[].text".to_string())?;

    log::debug!(
        "OP.GG MCP TFT 原始响应: {}",
        &content_text[..content_text.len().min(500)]
    );

    let mut parsed: serde_json::Value = serde_json::from_str(content_text)
        .map_err(|e| format!("解析 OP.GG MCP 数据失败: {}", e))?;

    // 附加 TFT 羁绊中文名称映射 + 英雄图标映射 + 英雄中文名称映射（LCU 优先，CDragon 兜底）
    attach_tft_meta_maps(&mut parsed).await;

    // 写入内存缓存（缓存的是含映射的完整 payload；命中时仍会再刷新一次映射）
    crate::lcu::opgg::put_cached(cache_key, parsed.clone());

    Ok(parsed)
}
