/**
 * 应用自定义 Tauri 事件契约（Rust emit ↔ 前端 listen）。
 *
 * 约定：
 * - 事件名与 Rust `emit("...")` 字符串保持一致，禁止散落硬编码
 * - payload 只存稳定字段（如 autoTag 用 carry/feeder key，不用中文展示名）
 * - LCU 原始 WS 事件 `lcu-ws-event` 仍透传 envelope，不在本文件收窄 data 结构
 *
 * 与 `scripts/check-commands.mjs` 互补：脚本校验事件名存在性，本文件约束 payload。
 */

/** LCU 客户端 / WS 生命周期 */
export const LcuLifecycleEvents = {
  ClientStarted: "lcu-client-started",
  ClientEnded: "lcu-client-ended",
  WsConnected: "lcu-ws-connected",
  WsDisconnected: "lcu-ws-disconnected",
  WsError: "lcu-ws-error",
  WsEvent: "lcu-ws-event",
} as const;

/** 上传 */
export const UploadEvents = {
  Success: "upload-success",
} as const;

export interface UploadSuccessPayload {
  gameId: number;
}

/** 自动打标 */
export const AutoTagEvents = {
  PlayerTagged: "auto-player-tagged",
} as const;

export interface AutoPlayerTaggedPayload {
  gameId: number;
  count: number;
  tags: Array<{
    puuid: string;
    /** 稳定 key：carry | carryDamage | feeder | inter | carried */
    tag: string;
    reason: string;
  }>;
}

/** OP.GG 构建就绪 */
export const OpggEvents = {
  BuildReady: "opgg-build-ready",
} as const;

export interface OpggBuildReadyPayload {
  championId: number;
  mode: string;
  position: string;
  _opggShowChampionId?: number;
}

/** 截图 */
export const ScreenshotEvents = {
  Saved: "screenshot-saved",
} as const;

/** 静态资源预加载完成 */
export const GameDataEvents = {
  Ready: "game-data-ready",
} as const;

/** SignalR 连接状态 */
export const SignalrEvents = {
  Connecting: "signalr-connecting",
  Connected: "signalr-connected",
  Disconnected: "signalr-disconnected",
  Error: "signalr-error",
} as const;

/** 雷达提醒 */
export const RadarEvents = {
  Alert: "radar-alert",
} as const;

export interface RadarAlertPayload {
  players: Array<{
    puuid?: string;
    name?: string;
    tag?: string | null;
    [key: string]: unknown;
  }>;
}

/** 托盘导航 */
export const TrayEvents = {
  Navigate: "tray-navigate",
} as const;

/** 更新器 */
export const UpdaterEvents = {
  UpdateAvailable: "updater://update-available",
  Progress: "updater://progress",
  DownloadReady: "updater://download-ready",
  DownloadError: "updater://download-error",
} as const;

/** 管道统计快照（get_pipeline_stats） */
export interface PipelineStatsSnapshot {
  champSelectThrottled: number;
  bpChannelDropped: number;
  matchChannelDropped: number;
  wsEventsEmitted: number;
  matchDetailCacheHits: number;
  matchDetailCacheMisses: number;
  uploadEnqueued: number;
  uploadSuccess: number;
  uploadFailed: number;
}
