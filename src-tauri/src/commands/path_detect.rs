use std::path::{Path, PathBuf};

/// 自动检测 LOL 客户端安装路径（从运行中的 LeagueClientUx.exe 推断，或从 Windows 注册表兜底）
#[tauri::command]
pub async fn detect_lol_path() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        // 1. 优先从运行中的客户端进程推断
        for process in sys.processes().values() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name == "leagueclientux.exe" {
                if let Some(exe_path) = process.exe() {
                    let mut dir = exe_path.parent();
                    while let Some(d) = dir {
                        if d.join("LeagueClient.exe").exists()
                            || d.join("Client.exe").exists()
                            || d.join("client.exe").exists()
                        {
                            return Ok(Some(d.to_string_lossy().to_string()));
                        }
                        dir = d.parent();
                    }
                    // 兜底：返回 exe 的上两级
                    if let Some(parent) = exe_path.parent() {
                        if let Some(root) = parent.parent() {
                            return Ok(Some(root.to_string_lossy().to_string()));
                        }
                    }
                }
            }
        }

        // 2. 进程未运行，则按照 Python 逻辑，尝试从 Windows 注册表获取国服 LOL 路径
        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("reg")
                .args(["query", r"HKCU\SOFTWARE\Tencent\LOL", "/v", "InstallPath"])
                .output()
            {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if line.contains("InstallPath") {
                            if let Some(pos) = line.find("REG_SZ") {
                                let raw_path = line[pos + 6..].trim();
                                if !raw_path.is_empty() {
                                    // 统一成正斜杠格式，规范盘符大小写
                                    let mut path = raw_path.replace("\\", "/");
                                    if path.len() >= 2 && path.as_bytes()[1] == b':' {
                                        let drive =
                                            path.chars().next().unwrap().to_uppercase().to_string();
                                        path = format!("{}{}", drive, &path[1..]);
                                    }

                                    // 如果是国服，注册表读出来的安装目录下有 TCLS 目录
                                    let tcls_dir = std::path::Path::new(&path).join("TCLS");
                                    if tcls_dir.exists() {
                                        return Ok(Some(
                                            tcls_dir.to_string_lossy().replace("\\", "/"),
                                        ));
                                    }

                                    return Ok(Some(path));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    })
    .await
    .map_err(|e| format!("路径检测任务异常终止: {}", e))?
}

/// 从注册表读取指定键的 REG_SZ 字符串值（Windows 专用，其他平台返回 None）
fn reg_string_value(key: &str, name: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("reg")
            .args(["query", key, "/v", name])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(name) {
                if let Some(pos) = line.find("REG_SZ") {
                    let raw = line[pos + 6..].trim();
                    if !raw.is_empty() {
                        return Some(raw.replace("\\", "/"));
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (key, name);
    }
    None
}

/// 从注册表卸载项中查找 WeGame（DisplayName 含 WeGame 的项，读 InstallLocation 或 DisplayIcon）
fn find_wegame_from_uninstall() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let uninstall_roots = [
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ];
        for root in uninstall_roots {
            let list_output = match std::process::Command::new("reg")
                .args(["query", root])
                .output()
            {
                Ok(o) => o,
                Err(_) => continue,
            };
            if !list_output.status.success() {
                continue;
            }
            for line in String::from_utf8_lossy(&list_output.stdout).lines() {
                let subkey = line.trim();
                if !subkey.starts_with(root) || subkey.len() <= root.len() {
                    continue;
                }
                let name = match reg_string_value(subkey, "DisplayName") {
                    Some(n) => n,
                    None => continue,
                };
                if !name.to_lowercase().contains("wegame") {
                    continue;
                }
                // 优先 InstallLocation
                if let Some(dir) = reg_string_value(subkey, "InstallLocation") {
                    let exe = Path::new(&dir).join("wegame.exe");
                    if exe.exists() {
                        return Some(exe);
                    }
                }
                // 其次 DisplayIcon（可能是 "路径,图标序号" 或带引号，取逗号前的路径）
                if let Some(icon) = reg_string_value(subkey, "DisplayIcon") {
                    let clean = icon
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"');
                    if !clean.is_empty() {
                        let icon_path = Path::new(clean);
                        if icon_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                            == Some("wegame.exe".to_string())
                            && icon_path.exists()
                        {
                            return Some(icon_path.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        return None;
    }
    None
}

/// 探测 WeGame 可执行文件位置（注册表 InstallPath → 注册表卸载项 → 常见安装目录 → 运行进程），找不到返回 None
pub(crate) fn find_wegame_exe() -> Option<PathBuf> {
    // 1. 注册表 InstallPath（WeGame 可能装在用户级或系统级）
    for key in [
        r"HKCU\SOFTWARE\Tencent\WeGame",
        r"HKLM\SOFTWARE\Tencent\WeGame",
        r"HKLM\SOFTWARE\WOW6432Node\Tencent\WeGame",
    ] {
        if let Some(dir) = reg_string_value(key, "InstallPath") {
            let exe = Path::new(&dir).join("wegame.exe");
            if exe.exists() {
                return Some(exe);
            }
        }
    }

    // 2. 注册表卸载项（安装信息所在，含 DisplayName/InstallLocation/DisplayIcon）
    if let Some(exe) = find_wegame_from_uninstall() {
        return Some(exe);
    }

    // 3. 常见安装目录（遍历所有存在盘符的 Program Files / Program Files (x86)）
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if !Path::new(&drive).exists() {
            continue;
        }
        for sub_dir in ["Program Files", "Program Files (x86)"] {
            let exe = Path::new(&drive)
                .join(sub_dir)
                .join("WeGame")
                .join("wegame.exe");
            if exe.exists() {
                return Some(exe);
            }
        }
    }
    // 补充常见非标准安装目录（这些目录未必在注册表卸载项中，直接探测最稳妥）
    for dir in [r"D:\WeGame", r"E:\WeGame"] {
        let exe = Path::new(dir).join("wegame.exe");
        if exe.exists() {
            return Some(exe);
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let exe = Path::new(&local).join("WeGame").join("wegame.exe");
        if exe.exists() {
            return Some(exe);
        }
    }

    // 4. 进程兜底：WeGame 运行中时从其 exe 路径推断
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        for process in sys.processes().values() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name == "wegame.exe" || name == "wegame" {
                if let Some(exe_path) = process.exe() {
                    return Some(exe_path.to_path_buf());
                }
            }
        }
    }

    None
}

/// 自动检测 WeGame 安装位置（找不到返回 None，供前端「添加 WeGame 启动项」使用）
#[tauri::command]
pub async fn detect_wegame_path() -> Result<Option<String>, String> {
    let exe = tauri::async_runtime::spawn_blocking(find_wegame_exe)
        .await
        .map_err(|e| format!("WeGame 检测任务异常终止: {}", e))?;
    Ok(exe.map(|exe| {
        exe.parent()
            .map(|d| d.to_string_lossy().replace("\\", "/"))
            .unwrap_or_else(|| exe.to_string_lossy().to_string())
    }))
}

/// 打开原生文件夹选择对话框，返回用户选择的路径
#[tauri::command]
pub async fn select_lol_folder() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let folder = rfd::FileDialog::new()
            .set_title("选择英雄联盟客户端安装目录")
            .pick_folder();
        Ok(folder.map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("文件夹选择任务异常终止: {}", e))?
}

/// 打开原生文件夹选择对话框，支持自定义标题和默认起始目录，返回用户选择的路径
#[tauri::command]
pub async fn select_folder(
    title: Option<String>,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new();
        if let Some(t) = title {
            dialog = dialog.set_title(&t);
        }

        // 确定起始定位的目录
        let mut start_path = None;
        if let Some(ref dp) = default_path {
            if !dp.is_empty() {
                start_path = Some(std::path::PathBuf::from(dp));
            }
        }

        // 如果没有指定（或者为空），则使用默认的 "图片/Yuumi_Screenshots" 目录
        let path_to_set = match start_path {
            Some(p) => p,
            None => {
                if let Some(mut p) = dirs::picture_dir() {
                    p.push("Yuumi_Screenshots");
                    let _ = std::fs::create_dir_all(&p); // 确保它存在
                    p
                } else {
                    std::path::PathBuf::new()
                }
            }
        };

        if path_to_set.exists() {
            dialog = dialog.set_directory(path_to_set);
        }

        let folder = dialog.pick_folder();
        Ok(folder.map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("文件夹选择任务异常终止: {}", e))?
}
