// ─── Tauri 命令薄封装层 ───
// 按业务域拆分：path_detect / client_launch / window_ui / github / lobby / runes / skins / opgg / client_tools / spectate / config / lcu。
pub mod client_launch;
pub mod client_tools;
pub mod config;
pub mod github;
pub mod lcu;
pub mod lobby;
pub mod opgg;
pub mod path_detect;
pub mod runes;
pub mod skins;
pub mod spectate;
pub mod window_ui;
