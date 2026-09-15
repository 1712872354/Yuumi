use crate::AppState;
use tauri::window::Effect;
use tauri::Manager;

/// 在系统文件管理器中打开截图保存的目录
#[tauri::command]
pub async fn open_screenshot_folder(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let config_lock = state.config.read().await;
    let custom_path = &config_lock.functions.screenshot_save_path;

    let path = if !custom_path.is_empty() {
        std::path::PathBuf::from(custom_path)
    } else {
        let mut p = dirs::picture_dir().ok_or_else(|| "无法获取系统图片目录".to_string())?;
        p.push("Yuumi_Screenshots");
        p
    };

    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 运行时切换云母效果
#[tauri::command]
pub fn set_mica_effect(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if enabled {
            window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![Effect::Mica],
                    state: None,
                    radius: None,
                    color: None,
                })
                .map_err(|e| e.to_string())?;
        } else {
            window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![],
                    state: None,
                    radius: None,
                    color: None,
                })
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 大乱斗板凳席置顶悬浮窗控制命令
#[tauri::command]
pub async fn show_bench_overlay_window(
    app_handle: tauri::AppHandle,
    show: bool,
) -> Result<(), String> {
    let window = app_handle.get_webview_window("bench-overlay");
    if show {
        if let Some(win) = window {
            let _ = win.show();
            let _ = win.set_focus();
            if let Ok(Some(monitor)) = win.current_monitor() {
                let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                let x = pos.x + (size.width - 550.0) / 2.0;
                let y = pos.y; // 动态定位至该显示器的最顶端
                let _ =
                    win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
            }
        } else {
            // 计算默认的顶部居中位置
            let mut x = 0.0;
            let mut y = 0.0;
            if let Ok(Some(monitor)) = app_handle.primary_monitor() {
                let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                x = pos.x + (size.width - 550.0) / 2.0;
                y = pos.y; // 动态定位至主显示器的最顶端
            }

            let win = tauri::WebviewWindowBuilder::new(
                &app_handle,
                "bench-overlay",
                tauri::WebviewUrl::App("index.html?window=bench-overlay".into()),
            )
            .title("Yuumi - ARAM Bench")
            .inner_size(550.0, 70.0)
            .position(x, y)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .build()
            .map_err(|e| e.to_string())?;

            let _ = win.show();
        }
    } else {
        if let Some(win) = window {
            let _ = win.close();
        }
    }
    Ok(())
}
