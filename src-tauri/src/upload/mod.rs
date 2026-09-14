//! 对局上传模块：队列、暂存、阶段触发、payload 构建与 Tauri 命令。

pub mod commands;
mod payload;
mod queue;
mod service;
mod store;
mod trigger;
mod url;

#[cfg(test)]
mod tests;

pub use commands::{batch_upload_matches, upload_single_match};
pub use queue::UploadQueue;
pub use service::BatchUploadResult;
pub use trigger::UploadTrigger;
