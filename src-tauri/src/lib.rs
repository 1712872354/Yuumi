pub mod agents;
pub mod auto_tag;
pub mod commands;
pub mod config;
pub mod lcu;
pub mod lcu_ops;
pub mod logging;
pub mod loot;
pub mod parsers;
pub mod portable_updater;
pub mod runtime;
pub mod saved_players;
pub mod signalr;
pub mod state;
pub mod updater;
pub mod upload;

use crate::state::{AgentRuntime, BenchRuntime, LcuRuntime, SignalrRuntime, UpdaterRuntime};
use base64::Engine;
use lcu::client::TauriBuilderExt;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Effect;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::{mpsc, RwLock};

/// 包装 tauri::async_runtime::spawn，捕获并记录后台任务异常终止。
/// 不会丢失 JoinHandle 的错误信息，避免任务静默崩溃。
pub(crate) fn spawn_log_panic<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let handle = tauri::async_runtime::spawn(future);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = handle.await {
            log::error!("后台任务异常终止: {:?}", e);
        }
    });
}

/// LCU 连接凭证及预配置的 HTTP Client
pub struct LcuClient {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    /// 登录大区标识（来自 LeagueClientUx 命令行 --rso_platform_id=），
    /// 用于 SGP 观战等需要大区信息的场景。非腾讯大区时为 None。
    pub server: Option<String>,
    pub http_client: reqwest::Client,
}

/// 供 Tauri 管理的全局状态。
/// 高频跨域字段（config 等）挂顶层；LCU / agent / 更新 / 板凳席按域聚合到 `state.rs`。
pub struct AppState {
    /// LCU 连接、静态资源、API 并发、WS 取消
    pub lcu: LcuRuntime,
    pub config: Arc<RwLock<config::AppConfig>>,
    /// BP / 游戏流程 agent 通道与竞态控制
    pub agents: AgentRuntime,
    /// 上传队列（可用于外部手动触发上传）
    pub upload_queue: Arc<upload::UploadQueue>,
    /// 自动更新下载/安装状态
    pub updater: UpdaterRuntime,
    /// 大乱斗板凳席悬浮窗缓存
    pub bench: BenchRuntime,
    /// SignalR Hub 运行时
    pub signalr: SignalrRuntime,
    /// SQLite 连接（保存的玩家）
    pub saved_db: Arc<Mutex<rusqlite::Connection>>,
    /// 当前对局信息缓存（对局结束记录相遇时使用）
    pub current_game_cache: Mutex<Option<saved_players::CurrentGameCache>>,
}

/// LCU 连接参数快照（读锁释放后使用，http_client 克隆是 Arc 浅拷贝）
pub struct LcuParams {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub server: Option<String>,
    pub http_client: reqwest::Client,
}

impl AppState {
    /// 获取 LCU 连接读锁，未连接时返回错误
    pub async fn lcu(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<LcuClient>>, String> {
        let lock = self.lcu.client.read().await;
        if lock.is_some() {
            Ok(lock)
        } else {
            Err("LCU 未连接，请先启动英雄联盟客户端".to_string())
        }
    }

    /// 提取 LCU 连接参数并尽早释放读锁，避免 HTTP 请求期间阻塞 monitor 重连。
    pub async fn lcu_params(&self) -> Result<LcuParams, String> {
        let lock = self.lcu().await?;
        let lcu = lock
            .as_ref()
            .ok_or_else(|| "LCU 未连接，请先启动英雄联盟客户端".to_string())?;
        Ok(LcuParams {
            pid: lcu.pid,
            port: lcu.port,
            token: lcu.token.clone(),
            server: lcu.server.clone(),
            http_client: lcu.http_client.clone(),
        })
    }
}

/// 激活主窗口（取消最小化/显示/聚焦）并在配置启用时重新应用云母效果，
/// 防止窗口长时间隐藏后 Mica 特效失效。托盘菜单与托盘图标点击共用。
fn activate_main_window_with_mica(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        // 重新应用云母效果以防失效
        let state = app.state::<AppState>();
        let is_mica_enabled = state
            .config
            .try_read()
            .map(|cfg| cfg.personalization.mica_enabled)
            .unwrap_or(false);
        if is_mica_enabled {
            let _ = crate::commands::os_shell::set_mica_effect(app.clone(), true);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 便携版：清理上次更新残留的临时目录（不阻塞）
    portable_updater::schedule_cleanup_from_environment();

    // 便携模式下动态化 identifier，避免与安装版（或多份便携副本）抢占单实例互斥锁
    let mut context = tauri::generate_context!();
    if runtime::is_portable() {
        context.config_mut().identifier =
            format!("com.yuumi.app.portable.{}", runtime::portable_instance_id());
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let _ = app.emit("single-instance", (argv, cwd));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        // updater 插件无条件注册：安装版走 NSIS 安装包，便携版复用同一插件
        // 通过 updater_builder().target("windows-x86_64-portable") 拉取便携 zip
        .plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .register_asset_protocol()
        .manage(portable_updater::PortableUpdateState::default())
        .setup(|app| {
            // 加载配置并做 clamp 限制，防止因 api_concurrency_number 为 0 导致请求挂起
            let mut app_config = config::AppConfig::load();
            if !(1..=32).contains(&app_config.functions.api_concurrency_number) {
                let clamped = app_config.functions.api_concurrency_number.clamp(1, 32);
                log::warn!(
                    "配置的 API 并发数 {} 不在 1..=32 范围内，已自动调整为 {}",
                    app_config.functions.api_concurrency_number,
                    clamped
                );
                app_config.functions.api_concurrency_number = clamped;
                app_config.save();
            }
            let api_concurrency = app_config.functions.api_concurrency_number as usize;

            let app_config_arc = Arc::new(RwLock::new(app_config));
            log::info!("配置已加载");

            // 创建 agent 通信 channels
            let (bp_tx, bp_rx) = mpsc::channel::<agents::auto_bp::ChampSelectSession>(32);
            let (gameflow_tx, gameflow_rx) = mpsc::channel::<agents::auto_match::GameflowEvent>(32);

            // 创建上传队列
            let upload_queue = Arc::new(upload::UploadQueue::new(app.handle().clone()));
            let upload_trigger = upload::UploadTrigger::new(upload_queue.clone());

            // 初始化全局状态
            let lcu_runtime = LcuRuntime::new(api_concurrency);
            let lcu_state = lcu_runtime.client.clone();
            let game_data = lcu_runtime.game_data.clone();
            let saved_db = match saved_players::init_db() {
                Ok(conn) => {
                    log::info!("SQLite 数据库已就绪");
                    Arc::new(Mutex::new(conn))
                }
                Err(e) => {
                    log::error!("打开 SQLite 数据库失败，保存的玩家功能不可用: {}", e);
                    Arc::new(Mutex::new(
                        rusqlite::Connection::open_in_memory().expect("内存库创建失败"),
                    ))
                }
            };
            let state = AppState {
                lcu: lcu_runtime,
                config: app_config_arc.clone(),
                agents: AgentRuntime::new(bp_tx, gameflow_tx),
                upload_queue,
                updater: UpdaterRuntime::default(),
                bench: BenchRuntime::default(),
                signalr: SignalrRuntime::default(),
                saved_db,
                current_game_cache: Mutex::new(None),
            };
            app.manage(state);

            // 启动 Agents
            agents::auto_bp::start(app.handle().clone(), bp_rx);
            agents::auto_match::start(app.handle().clone(), gameflow_rx, upload_trigger);
            agents::auto_screenshot::start(app.handle().clone());

            // 启动 LCU 进程监测
            let app_handle = app.handle().clone();
            lcu::monitor::start(app_handle, lcu_state, game_data);

            // 条件启动 SignalR Hub 远程反代
            {
                let cfg_snapshot = app_config_arc.blocking_read();
                let general = &cfg_snapshot.general;
                let functions = &cfg_snapshot.functions;
                if functions.lcu_realtime_enabled && !general.upload_api_url.is_empty() {
                    let server_url = general.upload_api_url.clone();
                    let user_id = cfg_snapshot.signalr_user_id();
                    log::info!("启动 SignalR Hub 远程反代");
                    signalr::start(app.handle().clone(), server_url, user_id);
                }
            }

            // 开发模式下自动打开 DevTools
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // ─── 云母效果 (Win11 Mica) ───
            {
                let cfg_snapshot = app_config_arc.blocking_read();
                if cfg_snapshot.personalization.mica_enabled {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.set_effects(tauri::utils::config::WindowEffectsConfig {
                            effects: vec![Effect::Mica],
                            state: None,
                            radius: None,
                            color: None,
                        });
                        log::info!("已启用云母效果 (Mica)");
                    }
                }
            }

            // ─── 系统托盘 ───
            let hide_tft = app_config_arc.blocking_read().functions.hide_tft;
            let tray_menu = crate::commands::config::build_tray_menu(app.handle(), hide_tft)?;

            let _tray = TrayIconBuilder::with_id("main_tray")
                .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
                    log::warn!("default_window_icon 为 None，使用 1x1 透明像素占位");
                    tauri::image::Image::new(&[0, 0, 0, 0], 1, 1)
                }))
                .menu(&tray_menu)
                .tooltip("Yuumi")
                .on_menu_event(
                    move |app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                        let id = event.id().as_ref().to_string();
                        if id == "exit" {
                            app.exit(0);
                        } else {
                            activate_main_window_with_mica(app);
                            let _ = app.emit("tray-navigate", &id);
                        }
                    },
                )
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event: TrayIconEvent| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        activate_main_window_with_mica(tray.app_handle());
                    }
                })
                .build(app)?;

            // ─── 启动时无条件静默检查更新（下载行为由 EnableCheckUpdate 区分）───
            {
                let app_handle = app.handle().clone();
                crate::spawn_log_panic(async move {
                    // 延迟 3 秒，等待主窗口完全加载后再检查
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if runtime::is_portable() {
                        // 便携版：仅检查并通知前端，下载由用户点击触发
                        portable_updater::start_background_check(app_handle).await;
                    } else {
                        // 安装版：无条件检查；若开启自动更新则后台静默下载，否则仅通知
                        updater::startup_check_update(app_handle).await;
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            lcu::client::call_lcu_api,
            lcu::client::get_lcu_asset,
            lcu::client::get_lcu_assets,
            parsers::summoner::get_current_summoner,
            parsers::match_parser::history::get_match_history,
            parsers::match_parser::history::get_match_history_sgp,
            parsers::match_parser::history::get_match_history_merged,
            parsers::match_parser::teammates::get_recent_teammates,
            parsers::game_info::get_player_fate_info,
            parsers::tft::data::get_tft_data,
            parsers::tft::rank::get_tft_ranked_stats,
            parsers::tft::history::get_tft_match_history,
            parsers::tft::augments::get_tft_augments,
            lcu_ops::create_5v5_practice_lobby,
            lcu_ops::aram_reroll_and_swap_back,
            lcu_ops::apply_rune_page,
            lcu_ops::get_lcu_zoom,
            lcu_ops::fix_lcu_window,
            lcu_ops::clear_game_cache,
            lcu_ops::open_log_folder,
            lcu_ops::fetch_opgg_data,
            lcu_ops::fetch_tft_meta_decks,
            lcu_ops::get_champion_skins,
            lcu_ops::get_game_settings_readonly,
            lcu_ops::set_game_settings_readonly,
            lcu_ops::spectate_directly,
            loot::open::get_openable_loots,
            loot::open::batch_open_loots,
            loot::open::smart_open_all_loots,
            loot::inventory::get_loot_inventory,
            loot::actions::disenchant_loot,
            loot::actions::reroll_loot,
            loot::actions::upgrade_loot,
            loot::inventory::get_essence_balances,
            commands::config::get_config,
            commands::config::update_config,
            commands::config::get_config_load_error,
            commands::config::get_close_to_tray,
            commands::lcu::get_lcu_connection_info,
            commands::lcu::get_map_side,
            commands::os_shell::detect_lol_path,
            commands::os_shell::detect_wegame_path,
            commands::os_shell::select_lol_folder,
            commands::os_shell::select_folder,
            commands::os_shell::open_screenshot_folder,
            commands::os_shell::set_mica_effect,
            commands::os_shell::launch_lol_client,
            commands::lcu::get_game_data_assets,
            commands::lcu::get_bench_my_champions,
            commands::lcu::get_live_game_teams,
            commands::os_shell::fetch_github_text,
            commands::os_shell::get_release_changelog,
            upload::commands::upload_single_match,
            upload::commands::batch_upload_matches,
            signalr::get_signalr_status,
            updater::check_update,
            updater::install_update,
            updater::install_pending_update,
            portable_updater::check_portable_update,
            portable_updater::download_portable_update,
            portable_updater::apply_portable_update,
            commands::os_shell::show_bench_overlay_window,
            saved_players::commands::save_saved_player,
            saved_players::commands::set_player_list_kind,
            saved_players::commands::query_all_saved_players,
            saved_players::commands::query_encountered_games,
            saved_players::commands::get_saved_players_map,
            saved_players::commands::delete_saved_player,
            saved_players::import_export::export_tagged_players_to_json_file,
            saved_players::import_export::import_tagged_players_from_json_file,
            saved_players::import_export::backfill_saved_player_identity,
            runtime::is_portable,
        ])
        .run(context)
        .expect("error while running tauri application");
}

/// 便携版更新 helper 进程入口判定（main() 第一行调用）。
/// 返回 true 表示当前进程是更新 helper 且已处理完毕，main 应立即 return。
pub fn run_portable_update_helper_if_requested() -> bool {
    portable_updater::run_helper_if_requested()
}

/// 构建 LCU Basic Auth header 值
pub fn build_auth_header(token: &str) -> String {
    let credentials = format!("riot:{}", token);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&credentials);
    format!("Basic {}", encoded)
}
