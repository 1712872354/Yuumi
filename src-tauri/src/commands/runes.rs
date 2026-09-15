use serde::Deserialize;
use tauri::State;

use crate::{lcu::client::lcu_request, AppState};

#[derive(Deserialize)]
pub struct RunePageParams {
    pub name: String,
    pub primary_style_id: i32,
    pub sub_style_id: i32,
    pub selected_perk_ids: Vec<i32>,
}

/// 一键应用符文页：获取当前 → 删除 → 创建新页
#[tauri::command]
pub async fn apply_rune_page(
    params: RunePageParams,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    apply_rune_page_core(
        app_state.inner(),
        &params.name,
        params.primary_style_id,
        params.sub_style_id,
        &params.selected_perk_ids,
    )
    .await?;
    Ok("符文页已应用".to_string())
}

/// 应用符文页核心逻辑（供 command 与自动选人共用）
pub(crate) async fn apply_rune_page_core(
    app_state: &AppState,
    name: &str,
    primary_style_id: i32,
    sub_style_id: i32,
    selected_perk_ids: &[i32],
) -> Result<(), String> {
    // 记录当前符文页：可删除时先删除腾出页位（页位已满时直接新建会失败），
    // 但删除前保留快照，若随后新建失败则恢复原页，避免丢失用户符文页
    let current_page = lcu_request(app_state, "GET", "/lol-perks/v1/currentpage", None)
        .await
        .ok();
    let deletable_old_page_id = current_page.as_ref().and_then(|page| {
        let deletable = page.get("isDeletable").and_then(|v| v.as_bool())?;
        let page_id = page.get("id").and_then(|v| v.as_i64())?;
        (deletable && page_id > 0).then_some(page_id)
    });

    let old_page_snapshot: Option<serde_json::Value> = match deletable_old_page_id {
        Some(page_id) => lcu_request(
            app_state,
            "GET",
            &format!("/lol-perks/v1/pages/{}", page_id),
            None,
        )
        .await
        .ok(),
        None => None,
    };

    if let Some(page_id) = deletable_old_page_id {
        let _ = lcu_request(
            app_state,
            "DELETE",
            &format!("/lol-perks/v1/pages/{}", page_id),
            None,
        )
        .await;
    }

    let body = serde_json::json!({
        "name": name,
        "primaryStyleId": primary_style_id,
        "subStyleId": sub_style_id,
        "selectedPerkIds": selected_perk_ids,
        "current": true,
    });
    if let Err(e) = lcu_request(app_state, "POST", "/lol-perks/v1/pages", Some(body)).await {
        // 新建失败：尽力恢复刚删除的原页
        if let Some(mut snapshot) = old_page_snapshot {
            log::warn!("应用符文页失败 ({})，尝试恢复原符文页", e);
            if let Some(obj) = snapshot.as_object_mut() {
                obj.remove("id");
                obj.remove("current");
            }
            if let Err(re) =
                lcu_request(app_state, "POST", "/lol-perks/v1/pages", Some(snapshot)).await
            {
                log::error!("恢复原符文页失败: {}", re);
            }
        }
        return Err(e);
    }
    Ok(())
}
