use tauri::State;

use crate::{build_auth_header, AppState};

#[derive(serde::Serialize)]
pub struct SkinEntry {
    pub id: i32,
    pub name: String,
    pub load_screen_path: String,
}

#[derive(serde::Deserialize)]
struct LcuSkin {
    id: i32,
    name: String,
    #[serde(rename = "loadScreenPath")]
    load_screen_path: Option<String>,
}

#[derive(serde::Deserialize)]
struct LcuChampionDetails {
    skins: Vec<LcuSkin>,
}

/// 根据英雄 ID 获取皮肤列表 (直接从 LCU 静态资源加载)
#[tauri::command]
pub async fn get_champion_skins(
    champion_id: i32,
    app_state: State<'_, AppState>,
) -> Result<Vec<SkinEntry>, String> {
    // 尽早释放读锁，避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client) = (lcu.port, lcu.token, lcu.http_client);
    let auth = build_auth_header(&token);
    let base = format!("https://127.0.0.1:{}", port);

    let url = format!(
        "{}/lol-game-data/assets/v1/champions/{}.json",
        base, champion_id
    );
    let resp = http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!(
            "LCU 返回错误 [{}]: 无法加载该英雄的皮肤数据",
            resp.status().as_u16()
        ));
    }

    let details: LcuChampionDetails = resp.json().await.map_err(|e| e.to_string())?;

    let skins = details
        .skins
        .into_iter()
        .map(|s| SkinEntry {
            id: s.id,
            name: s.name,
            load_screen_path: s.load_screen_path.unwrap_or_else(|| {
                format!(
                    "/lol-game-data/assets/v1/champion-loadscreens/{}/{}.jpg",
                    champion_id, s.id
                )
            }),
        })
        .collect();

    Ok(skins)
}
