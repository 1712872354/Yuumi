use tauri::{Emitter, State};

use super::*;
use crate::{build_auth_header, AppState};

/// 5. 批量分解选中碎片（后台顺序执行，逐条广播进度）
#[tauri::command]
pub async fn disenchant_loot(
    items: Vec<DisenchantItem>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lcu = app_state.lcu_params().await?;
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client;

    crate::spawn_log_panic(async move {
        let total_jobs = items.len() as i32;

        for (current_job, job) in items.into_iter().enumerate() {
            let current_job = current_job as i32 + 1;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            // 1. 动态查找分解配方
            match find_recipe_name(&base, &auth, &http_client, &job.loot_id, "disenchant").await {
                Ok(recipe_name) => {
                    // 2. 执行分解请求
                    let craft_url = format!(
                        "{}/lol-loot/v1/recipes/{}/craft?repeat={}",
                        base, recipe_name, job.count
                    );
                    let body = vec![job.loot_id.clone()];

                    let craft_resp = http_client
                        .post(&craft_url)
                        .header("Authorization", &auth)
                        .json(&body)
                        .send()
                        .await;

                    match craft_resp {
                        Ok(r) if r.status().is_success() => {
                            let _ = app_handle.emit(
                                "loot-disenchant-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: true,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: format!("成功分解 {} 个", job.count),
                                    error_msg: None,
                                },
                            );
                        }
                        Ok(r) => {
                            let _ = app_handle.emit(
                                "loot-disenchant-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(format!("HTTP 错误: {}", r.status())),
                                },
                            );
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "loot-disenchant-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(e.to_string()),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-disenchant-progress",
                        ActionProgressEvent {
                            current: current_job,
                            total: total_jobs,
                            success: false,
                            loot_name: job.loot_id,
                            reward_desc: String::new(),
                            error_msg: Some(format!("找不到配方: {}", e)),
                        },
                    );
                }
            }
        }
    });

    Ok("批量分解任务已推入后台队列".to_string())
}

/// 6. 批量三合一重随：输入平铺 loot_ids 列表（长度需为 3 的倍数，前端按类别分包）
///    每 3 个为一组，调用对应重随/锻造配方合成一个永久物品。
#[tauri::command]
pub async fn reroll_loot(
    loot_ids: Vec<String>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    if loot_ids.is_empty() || !loot_ids.len().is_multiple_of(3) {
        return Err("重随必须提供3的倍数个物品".to_string());
    }

    let lcu = app_state.lcu_params().await?;
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client;

    crate::spawn_log_panic(async move {
        let total_groups = (loot_ids.len() / 3) as i32;

        for g in 0..total_groups {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            let idx = (g * 3) as usize;
            let group_ingredients = vec![
                loot_ids[idx].clone(),
                loot_ids[idx + 1].clone(),
                loot_ids[idx + 2].clone(),
            ];

            // 表情用 forge 配方，其他用 reroll 配方
            let keyword = if group_ingredients[0].contains("EMOTE") {
                "forge"
            } else {
                "reroll"
            };

            match find_recipe_name(&base, &auth, &http_client, &group_ingredients[0], keyword).await
            {
                Ok(recipe_name) => {
                    let craft_url = format!("{}/lol-loot/v1/recipes/{}/craft", base, recipe_name);

                    let craft_resp = http_client
                        .post(&craft_url)
                        .header("Authorization", &auth)
                        .json(&group_ingredients)
                        .send()
                        .await;

                    match craft_resp {
                        Ok(r) if r.status().is_success() => {
                            let mut reward_name = "未知永久物品".to_string();
                            if let Ok(detail) = r.json::<serde_json::Value>().await {
                                if let Some(added) = detail.get("added").and_then(|v| v.as_array())
                                {
                                    let rewards: Vec<String> = added
                                        .iter()
                                        .map(|x| {
                                            x.get("playerLoot")
                                                .and_then(|pl| pl.get("itemDesc"))
                                                .and_then(|d| d.as_str())
                                                .unwrap_or("")
                                                .to_string()
                                        })
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    if !rewards.is_empty() {
                                        reward_name = rewards.join(", ");
                                    }
                                }
                            }

                            let _ = app_handle.emit(
                                "loot-reroll-progress",
                                ActionProgressEvent {
                                    current: g + 1,
                                    total: total_groups,
                                    success: true,
                                    loot_name: group_ingredients[0].clone(),
                                    reward_desc: reward_name,
                                    error_msg: None,
                                },
                            );
                        }
                        Ok(r) => {
                            let _ = app_handle.emit(
                                "loot-reroll-progress",
                                ActionProgressEvent {
                                    current: g + 1,
                                    total: total_groups,
                                    success: false,
                                    loot_name: group_ingredients[0].clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(format!("HTTP 错误: {}", r.status())),
                                },
                            );
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "loot-reroll-progress",
                                ActionProgressEvent {
                                    current: g + 1,
                                    total: total_groups,
                                    success: false,
                                    loot_name: group_ingredients[0].clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(e.to_string()),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-reroll-progress",
                        ActionProgressEvent {
                            current: g + 1,
                            total: total_groups,
                            success: false,
                            loot_name: group_ingredients[0].clone(),
                            reward_desc: String::new(),
                            error_msg: Some(format!("找不到配方: {}", e)),
                        },
                    );
                }
            }
        }
    });

    Ok("三合一重随任务已推入后台队列".to_string())
}

/// 7. 批量升级选中项为永久物品
#[tauri::command]
pub async fn upgrade_loot(
    items: Vec<DisenchantItem>,
    app_handle: tauri::AppHandle,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lcu = app_state.lcu_params().await?;
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);
    let http_client = lcu.http_client;

    crate::spawn_log_panic(async move {
        let total_jobs = items.len() as i32;

        for (current_job, job) in items.into_iter().enumerate() {
            let current_job = current_job as i32 + 1;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;

            let loot_upper = job.loot_id.to_uppercase();
            // 不包含 SHARD 和 RENTAL 的为“永久战利品”，可以直接调用 LCU 自带的兑换接口直接解锁，无需配方
            let is_permanent = !loot_upper.contains("SHARD") && !loot_upper.contains("RENTAL");

            if is_permanent {
                let redeem_url = format!("{}/lol-loot/v1/player-loot/{}/redeem", base, job.loot_id);
                log::info!(
                    "[loot][upgrade] {} 是永久道具，直接调用兑换接口解锁: {}",
                    job.loot_id,
                    redeem_url
                );

                let mut success_count = 0;
                let mut last_error: Option<String> = None;

                for _ in 0..job.count {
                    match http_client
                        .post(&redeem_url)
                        .header("Authorization", &auth)
                        .send()
                        .await
                    {
                        Ok(r) if r.status().is_success() => {
                            success_count += 1;
                        }
                        Ok(r) => {
                            let status = r.status();
                            let body_text = r.text().await.unwrap_or_default();
                            log::warn!(
                                "[loot][upgrade] {} 兑换解锁失败 status={} body={}",
                                job.loot_id,
                                status,
                                body_text
                            );
                            let extra = if body_text.contains("CLIENT_ERROR") {
                                " (通常由于未拥有皮肤对应的英雄，或该永久道具已在当前账号拥有。)"
                            } else {
                                ""
                            };
                            last_error = Some(format!("HTTP {}: {}{}", status, body_text, extra));
                            break;
                        }
                        Err(e) => {
                            log::warn!("[loot][upgrade] {} 请求失败: {}", job.loot_id, e);
                            last_error = Some(e.to_string());
                            break;
                        }
                    }
                    if job.count > 1 {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }

                if last_error.is_none() {
                    let _ = app_handle.emit(
                        "loot-upgrade-progress",
                        ActionProgressEvent {
                            current: current_job,
                            total: total_jobs,
                            success: true,
                            loot_name: job.loot_id.clone(),
                            reward_desc: format!("成功解锁 {} 个", success_count),
                            error_msg: None,
                        },
                    );
                } else {
                    let _ = app_handle.emit(
                        "loot-upgrade-progress",
                        ActionProgressEvent {
                            current: current_job,
                            total: total_jobs,
                            success: false,
                            loot_name: job.loot_id.clone(),
                            reward_desc: String::new(),
                            error_msg: last_error,
                        },
                    );
                }
                continue;
            }

            // 1. 获取升级配方及其需要的额外材料 (优先使用前端传来的精确配方名，无则使用关键字兜底)
            let recipe_result = find_recipe_upgrade_info(
                &base,
                &auth,
                &http_client,
                &job.loot_id,
                job.upgrade_recipe_name.as_deref(),
            )
            .await;

            match recipe_result {
                Ok((recipe_name, extra_ingredients)) => {
                    // 2. 执行升级请求
                    let loot_upper = job.loot_id.to_uppercase();
                    let mut body = vec![job.loot_id.clone()];
                    for item in extra_ingredients {
                        body.push(item);
                    }

                    // 永恒星碑(STATSTONE)的 LCU 配方不支持 repeat>1，强制逐个调用
                    let repeat_times = if loot_upper.contains("STATSTONE") {
                        job.count
                    } else {
                        1
                    };
                    let repeat_param = if loot_upper.contains("STATSTONE") {
                        1
                    } else {
                        job.count
                    };

                    log::info!(
                        "[loot][upgrade] craft lootId={} recipe={} body={:?} repeat_param={} repeat_times={}",
                        job.loot_id, recipe_name, body, repeat_param, repeat_times
                    );

                    let mut success_count = 0;
                    let mut last_error: Option<String> = None;

                    for _ in 0..repeat_times {
                        let craft_url = format!(
                            "{}/lol-loot/v1/recipes/{}/craft?repeat={}",
                            base, recipe_name, repeat_param
                        );
                        match http_client
                            .post(&craft_url)
                            .header("Authorization", &auth)
                            .json(&body)
                            .send()
                            .await
                        {
                            Ok(r) if r.status().is_success() => {
                                success_count += 1;
                            }
                            Ok(r) => {
                                let status = r.status();
                                let body_text = r.text().await.unwrap_or_default();
                                log::warn!(
                                    "[loot][upgrade] {} 升级失败 status={} body={}",
                                    job.loot_id,
                                    status,
                                    body_text
                                );
                                let extra = if body_text.contains("CLIENT_ERROR") {
                                    " (通常由于精粹余额不足，或者该永久道具已在当前账号拥有。)"
                                } else {
                                    ""
                                };
                                last_error =
                                    Some(format!("HTTP {}: {}{}", status, body_text, extra));
                                break;
                            }
                            Err(e) => {
                                log::warn!("[loot][upgrade] {} 请求失败: {}", job.loot_id, e);
                                last_error = Some(e.to_string());
                                break;
                            }
                        }
                        if repeat_times > 1 {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }

                    let craft_resp: Result<(), String> = if last_error.is_none() {
                        Ok(())
                    } else {
                        Err(last_error.unwrap_or_default())
                    };

                    match craft_resp {
                        Ok(()) => {
                            let _ = app_handle.emit(
                                "loot-upgrade-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: true,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: format!("成功升级 {} 个", success_count),
                                    error_msg: None,
                                },
                            );
                        }
                        Err(e) => {
                            let _ = app_handle.emit(
                                "loot-upgrade-progress",
                                ActionProgressEvent {
                                    current: current_job,
                                    total: total_jobs,
                                    success: false,
                                    loot_name: job.loot_id.clone(),
                                    reward_desc: String::new(),
                                    error_msg: Some(e),
                                },
                            );
                        }
                    } // end match craft_resp
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "loot-upgrade-progress",
                        ActionProgressEvent {
                            current: current_job,
                            total: total_jobs,
                            success: false,
                            loot_name: job.loot_id,
                            reward_desc: String::new(),
                            error_msg: Some(format!("找不到升级配方: {}", e)),
                        },
                    );
                }
            }
        }
    });

    Ok("批量升级任务已推入后台队列".to_string())
}
