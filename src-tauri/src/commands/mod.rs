// ─── Tauri 命令薄封装层 ───
// 本模块仅包含对 Tauri invoke 暴露的轻量命令（文件选择、系统操作等）。
// 业务逻辑较重的命令放在顶层模块：`lcu_ops`（LCU 业务）、`os_shell`（系统/启动/GitHub）。
pub mod config;
pub mod lcu;
pub mod os_shell;
