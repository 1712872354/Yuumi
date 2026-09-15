use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::config::WEGAME_MARKER;
use crate::{build_auth_header, AppState};

/// 清除本地游戏资源缓存（头像、装备、技能、符文、强化图标）
#[tauri::command]
pub async fn clear_game_cache() -> Result<String, String> {
    let cache_dir = crate::runtime::app_data_dir().join("cache");

    if !cache_dir.exists() {
        return Ok("缓存目录不存在，无需清除".to_string());
    }

    let mut count = 0u32;
    for entry in std::fs::read_dir(&cache_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if std::fs::remove_dir_all(&path).is_ok() {
                count += 1;
            }
        } else if path.is_file() && std::fs::remove_file(&path).is_ok() {
            count += 1;
        }
    }

    Ok(format!("已清除 {} 个缓存文件/目录", count))
}

/// 打开日志文件夹
#[tauri::command]
pub async fn open_log_folder(app: tauri::AppHandle) -> Result<String, String> {
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("log")))
        .unwrap_or_else(|| std::path::PathBuf::from("log"));

    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    app.opener()
        .open_path(log_dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| e.to_string())?;

    Ok("已打开日志文件夹".to_string())
}

/// 获取当前 LCU 客户端缩放比例（用于窗口修复）
#[tauri::command]
pub async fn get_lcu_zoom(app_state: State<'_, AppState>) -> Result<f64, String> {
    // 尽早释放读锁，避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let lcu = app_state.lcu_params().await?;
    let (port, token, http_client) = (lcu.port, lcu.token, lcu.http_client);

    let url = format!("https://127.0.0.1:{}/riotclient/zoom-scale", port);
    let auth = build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        let zoom: f64 = resp.json().await.map_err(|e| e.to_string())?;
        Ok(zoom)
    } else {
        Err(format!("获取缩放失败: HTTP {}", resp.status()))
    }
}

/// 修复 LCU 客户端窗口（黑屏/缩放/转圈）。
/// 通过系统命令强制重新设置窗口属性。
#[tauri::command]
pub async fn fix_lcu_window(app_state: State<'_, AppState>) -> Result<String, String> {
    // 尽早释放读锁，避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let (pid, zoom) = {
        let lcu = app_state.lcu_params().await?;
        let url = format!("https://127.0.0.1:{}/riotclient/zoom-scale", lcu.port);
        let auth = build_auth_header(&lcu.token);
        let resp = lcu
            .http_client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let zoom = if resp.status().is_success() {
            resp.json::<f64>().await.map_err(|e| e.to_string())?
        } else {
            return Err(format!("获取缩放失败: HTTP {}", resp.status()));
        };
        (lcu.pid, zoom)
    };

    // 通过 Win32 API 直接操作窗口，替代旧的 PowerShell 脚本方案
    #[cfg(target_os = "windows")]
    {
        fix_lcu_window_win32(zoom, pid)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = zoom;
        Err("仅 Windows 平台支持窗口修复".to_string())
    }
}

#[cfg(target_os = "windows")]
fn fix_lcu_window_win32(zoom: f64, target_pid: u32) -> Result<String, String> {
    use std::ffi::c_void;
    use std::ptr;

    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
        fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
        fn SetWindowPos(
            hWnd: *mut c_void,
            hWndInsertAfter: *mut c_void,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
        fn GetWindowThreadProcessId(hWnd: *mut c_void, lpdwProcessId: *mut u32) -> u32;
        fn EnumWindows(
            lpEnumFunc: Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32>,
            lParam: *mut c_void,
        ) -> i32;
    }

    const SW_RESTORE: i32 = 9;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    unsafe {
        let mut hwnd = {
            let class_name: Vec<u16> = "RiotWindow\0".encode_utf16().collect();
            FindWindowW(class_name.as_ptr(), ptr::null())
        };

        if hwnd.is_null() {
            // 未直接命中 RiotWindow 时，按已知 pid 枚举窗口定位（复用 AppState 中的 pid，免全量进程扫描）
            if target_pid == 0 {
                return Err("未找到 LCU 窗口".to_string());
            }
            struct EnumData {
                target_pid: u32,
                hwnd: *mut c_void,
            }

            unsafe extern "system" fn enum_callback(hwnd: *mut c_void, lparam: *mut c_void) -> i32 {
                let data = &mut *(lparam as *mut EnumData);
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == data.target_pid {
                    data.hwnd = hwnd;
                    0
                } else {
                    1
                }
            }

            let mut data = EnumData {
                target_pid,
                hwnd: ptr::null_mut(),
            };
            EnumWindows(Some(enum_callback), (&mut data as *mut EnumData).cast());
            hwnd = data.hwnd;
        }

        if hwnd.is_null() {
            return Err("未找到 LCU 窗口".to_string());
        }

        ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );

        Ok(format!("窗口已修复 (zoom={})", zoom))
    }
}

fn get_persisted_settings_path(lol_paths: &[String]) -> Option<PathBuf> {
    // 跳过 WeGame 标记，取第一个真实客户端路径
    let real_path = lol_paths.iter().find(|p| *p != WEGAME_MARKER)?;
    let p = Path::new(real_path);
    let base_dir = if p.is_file() { p.parent()? } else { p };
    Some(
        base_dir
            .join("Game")
            .join("Config")
            .join("PersistedSettings.json"),
    )
}

/// 查询游戏设置（PersistedSettings.json）是否已被锁定（只读）
#[tauri::command]
pub async fn get_game_settings_readonly(app_state: State<'_, AppState>) -> Result<bool, String> {
    let cfg = app_state.config.read().await;
    let path = get_persisted_settings_path(&cfg.general.lol_path)
        .ok_or_else(|| "未配置英雄联盟客户端路径".to_string())?;

    if !path.exists() {
        return Ok(false);
    }

    let metadata = fs::metadata(&path).map_err(|e| format!("获取文件元数据失败: {}", e))?;

    Ok(metadata.permissions().readonly())
}

/// 锁定/解锁游戏设置（修改 PersistedSettings.json 的只读属性）
#[tauri::command]
pub async fn set_game_settings_readonly(
    readonly: bool,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let cfg = app_state.config.read().await;
    let path = get_persisted_settings_path(&cfg.general.lol_path)
        .ok_or_else(|| "未配置英雄联盟客户端路径".to_string())?;

    if !path.exists() {
        return Err(
            "游戏配置文件 PersistedSettings.json 不存在，请先登录一次游戏以自动生成该文件"
                .to_string(),
        );
    }

    let metadata = fs::metadata(&path).map_err(|e| format!("获取文件元数据失败: {}", e))?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(readonly);

    fs::set_permissions(&path, permissions).map_err(|e| format!("修改文件属性失败: {}", e))?;

    if readonly {
        Ok("游戏设置已锁定（只读状态）".to_string())
    } else {
        Ok("游戏设置已解锁（可读写状态）".to_string())
    }
}
