//! AppState 按域拆分的运行时状态结构。
//! LCU / agent / 更新 / 板凳席按域聚合；config 与 SQLite 仍挂在 AppState 顶层。

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use crate::agents;
use crate::lcu::game_data::GameDataAssets;
use crate::updater::{PendingUpdate, UpdateInfo};
use crate::LcuClient;
use tokio::sync::{mpsc, watch, RwLock, Semaphore};

/// LCU 连接、静态资源、API 并发与 WebSocket 取消信号。
pub struct LcuRuntime {
    /// LCU 连接凭证及预配置的 HTTP Client
    pub client: Arc<RwLock<Option<LcuClient>>>,
    /// LCU 连接后加载的游戏资源路径映射（物品/技能/符文 iconPath）
    pub game_data: Arc<RwLock<GameDataAssets>>,
    /// LCU API 并发信号量（由 config.ApiConcurrencyNumber 控制）
    pub api_semaphore: RwLock<Arc<Semaphore>>,
    /// WebSocket 连接取消信号发送端（新连接时发送取消旧循环）
    pub ws_cancel_tx: Mutex<Option<watch::Sender<bool>>>,
}

impl LcuRuntime {
    pub fn new(api_concurrency: usize) -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            game_data: Arc::new(RwLock::new(GameDataAssets::default())),
            api_semaphore: RwLock::new(Arc::new(Semaphore::new(api_concurrency))),
            ws_cancel_tx: Mutex::new(None),
        }
    }
}

/// BP / 游戏流程 agent 通道与竞态控制标志。
pub struct AgentRuntime {
    /// BP agent 的选人会话发送端
    pub bp_session_tx: mpsc::Sender<agents::auto_bp::ChampSelectSession>,
    /// 游戏流程 agent 的事件发送端
    pub gameflow_tx: mpsc::Sender<agents::auto_match::GameflowEvent>,
    /// BP 状态重置标志（gameflow 阶段变化时置为 true，BP agent 检查后置 false）
    pub bp_reset_flag: AtomicBool,
    /// BP 锁定后台任务版本号（用于标记和防止残留协程竞态）
    pub bp_task_id: AtomicU64,
}

impl AgentRuntime {
    pub fn new(
        bp_session_tx: mpsc::Sender<agents::auto_bp::ChampSelectSession>,
        gameflow_tx: mpsc::Sender<agents::auto_match::GameflowEvent>,
    ) -> Self {
        Self {
            bp_session_tx,
            gameflow_tx,
            bp_reset_flag: AtomicBool::new(false),
            bp_task_id: AtomicU64::new(0),
        }
    }
}

/// 自动更新下载/安装运行时状态。
pub struct UpdaterRuntime {
    /// 后台下载进行中标志，防止重复启动多个下载
    pub is_downloading: AtomicBool,
    /// 正在后台下载的更新信息
    pub downloading_update: Mutex<Option<UpdateInfo>>,
    /// 后台已下载完成的待安装更新
    pub pending_update: Mutex<Option<PendingUpdate>>,
}

impl Default for UpdaterRuntime {
    fn default() -> Self {
        Self {
            is_downloading: AtomicBool::new(false),
            downloading_update: Mutex::new(None),
            pending_update: Mutex::new(None),
        }
    }
}

/// SignalR Hub 连接运行时（命令通道 / 取消信号 / 当前召唤师名缓存）。
pub struct SignalrRuntime {
    pub tx: tokio::sync::Mutex<Option<mpsc::Sender<crate::signalr::SignalrCommand>>>,
    pub cancel_tx: tokio::sync::Mutex<Option<watch::Sender<bool>>>,
    pub current_summoner_name: tokio::sync::Mutex<String>,
}

impl Default for SignalrRuntime {
    fn default() -> Self {
        Self {
            tx: tokio::sync::Mutex::new(None),
            cancel_tx: tokio::sync::Mutex::new(None),
            current_summoner_name: tokio::sync::Mutex::new(String::new()),
        }
    }
}

/// 大乱斗板凳席悬浮窗相关缓存。
pub struct BenchRuntime {
    /// 本局当前玩家拥有过的英雄列表（悬浮窗挂载时主动拉取）
    pub my_champions: Mutex<Vec<i64>>,
    /// 上一次的 gameflow 阶段，避免阶段重复事件造成重复清空历史英雄缓存
    pub last_gameflow_phase: Mutex<String>,
}

impl Default for BenchRuntime {
    fn default() -> Self {
        Self {
            my_champions: Mutex::new(Vec::new()),
            last_gameflow_phase: Mutex::new(String::new()),
        }
    }
}
