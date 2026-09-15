use crate::config::WEGAME_MARKER;
use crate::AppState;
use std::path::Path;

use super::path_detect::find_wegame_exe;

/// 启动 WeGame 客户端。优先使用用户配置的路径（可能是目录或 exe 完整路径），
/// 未配置或路径失效时自动探测 wegame.exe 位置，找不到返回可读错误。
fn launch_wegame(configured_path: Option<&str>) -> Result<(), String> {
    if let Some(p) = configured_path {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            let path = Path::new(trimmed);
            let exe = if path.is_file() {
                path.to_path_buf()
            } else {
                path.join("wegame.exe")
            };
            if exe.exists() {
                log::info!("启动 WeGame (配置路径): {:?}", exe);
                return spawn_executable(&exe, &[]);
            }
            log::warn!("配置的 WeGame 路径无效 ({:?})，回退自动探测", exe);
        }
    }

    let exe = find_wegame_exe().ok_or_else(|| {
        "未找到 WeGame。请先安装 WeGame，或在设置中手动配置 WeGame 路径。".to_string()
    })?;
    log::info!("启动 WeGame (自动探测): {:?}", exe);
    spawn_executable(&exe, &[])
}

/// 启动可执行文件并处理 UAC 提升（设置 cwd 防止 DLL 加载报错；740/5 走管理员提权）
fn spawn_executable(exe: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    // 隔离子进程标准流：避免其（及其孙进程，如 WeGame 调用的 curl）输出混入 Yuumi 控制台/日志
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // 关键：设置启动工作目录为 exe 所在的父目录，防止 DLL 加载或配置读取报拒绝访问错误 (os error 5)
    if let Some(parent) = exe.parent() {
        cmd.current_dir(parent);
    }
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) => {
            let os_err = e.raw_os_error();
            // 拦截 740 (需要提升) 与 5 (拒绝访问) 并尝试以 UAC 管理员提权运行
            if os_err == Some(740) || os_err == Some(5) {
                log::info!("启动客户端遇到权限限制 ({:?})，尝试提升权限启动...", os_err);
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    let working_dir = exe
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let escape_ps_string = |s: &str| -> String { s.replace("'", "''") };

                    let escaped_exe = escape_ps_string(&exe.to_string_lossy());
                    let escaped_working_dir = escape_ps_string(&working_dir);

                    // 格式化参数传给 PowerShell
                    let args_str = args
                        .iter()
                        .map(|arg| format!("'{}'", escape_ps_string(arg)))
                        .collect::<Vec<String>>()
                        .join(", ");

                    let command_str = if args_str.is_empty() {
                        format!(
                            "Start-Process -FilePath '{}' -WorkingDirectory '{}' -Verb RunAs",
                            escaped_exe, escaped_working_dir
                        )
                    } else {
                        format!(
                            "Start-Process -FilePath '{}' -ArgumentList {} -WorkingDirectory '{}' -Verb RunAs",
                            escaped_exe,
                            args_str,
                            escaped_working_dir
                        )
                    };

                    let status = std::process::Command::new("powershell")
                        .creation_flags(0x08000000) // 隐藏 powershell 窗口
                        .args(["-Command", &command_str])
                        .spawn();
                    if status.is_ok() {
                        return Ok(());
                    }
                }
            }
            Err(format!("启动失败: {}", e))
        }
    }
}

/// 启动 LOL 客户端（指定路径或从配置中的 lol_path 查找）
#[tauri::command]
pub async fn launch_lol_client(
    app_state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    // 先检查是否已有客户端在运行（进程扫描为同步阻塞操作，放入阻塞线程池）
    let already_running = tokio::task::spawn_blocking(|| {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes().values().any(|p| {
            p.name()
                .to_string_lossy()
                .eq_ignore_ascii_case("leagueclientux.exe")
        })
    })
    .await
    .map_err(|e| format!("进程扫描任务异常终止: {}", e))?;
    if already_running {
        log::info!("客户端已在运行，跳过启动");
        return Ok(());
    }

    // 智能探测客户端执行文件的辅助函数
    let find_executable = |base_path: &str| -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(base_path);
        let check_list = &[
            ("", "LeagueClient.exe"),
            ("", "Client.exe"),
            ("", "client.exe"),
            ("TCLS", "client.exe"),
            ("TCLS", "Client.exe"),
            ("LeagueClient", "LeagueClient.exe"),
            ("../TCLS", "client.exe"),
            ("../TCLS", "Client.exe"),
            ("../LeagueClient", "LeagueClient.exe"),
        ];

        for (sub_dir, exe_name) in check_list {
            let exe = if sub_dir.is_empty() {
                p.join(exe_name)
            } else {
                p.join(sub_dir).join(exe_name)
            };
            if exe.exists() {
                return Some(exe);
            }
        }
        None
    };

    // 智能转换：若是 Riot 纳管的外服，改由 RiotClientServices.exe 启动
    let check_and_launch = |exe: std::path::PathBuf| -> Result<(), String> {
        let mut riot_service = None;
        if exe.file_name().map(|n| n.to_string_lossy().to_lowercase())
            == Some("leagueclient.exe".to_string())
        {
            if let Some(parent) = exe.parent() {
                let same_level = parent.join("RiotClientServices.exe");
                if same_level.exists() {
                    riot_service = Some(same_level);
                } else if let Some(grandparent) = parent.parent() {
                    let parent_level = grandparent.join("RiotClientServices.exe");
                    if parent_level.exists() {
                        riot_service = Some(parent_level);
                    }
                }
            }
        }

        if let Some(service) = riot_service {
            log::info!("检测到外服 Riot 纳管客户端，改由 RiotClientServices.exe 启动");
            let is_pbe = exe.to_string_lossy().to_lowercase().contains("pbe");
            let patchline = if is_pbe { "pbe" } else { "live" };
            let args = &[
                "--launch-product=league_of_legends",
                &format!("--launch-patchline={}", patchline),
            ];
            log::info!("启动 Riot 服务: {:?} {:?}", service, args);
            spawn_executable(&service, args)?;
        } else {
            log::info!("常规方式启动客户端: {:?}", exe);
            spawn_executable(&exe, &[])?;
        }
        Ok(())
    };

    // 指定了路径则直接用
    if let Some(p) = path {
        if p == WEGAME_MARKER {
            let cfg = app_state.config.read().await;
            return launch_wegame(cfg.general.wegame_path.as_deref());
        }
        if let Some(exe) = find_executable(&p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            check_and_launch(exe)?;
            return Ok(());
        }
        return Err(format!(
            "在 {} 中未找到启动程序 (TCLS/client.exe 或 LeagueClient.exe)",
            p
        ));
    }

    // 否则遍历配置路径
    let cfg = app_state.config.read().await;
    for p in &cfg.general.lol_path {
        if p == WEGAME_MARKER {
            return launch_wegame(cfg.general.wegame_path.as_deref());
        }
        if let Some(exe) = find_executable(p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            check_and_launch(exe)?;
            return Ok(());
        }
    }
    Err("未找到 LeagueClient.exe / Client.exe，请先在设置中配置客户端路径".to_string())
}
