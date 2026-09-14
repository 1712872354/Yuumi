import { invoke } from "@tauri-apps/api/core";
import type { PipelineStatsSnapshot } from "../../types/events";

/** 读取进程内管道统计（WS 节流/丢帧、详情缓存、上传队列） */
export const getPipelineStats = () =>
  invoke<PipelineStatsSnapshot>("get_pipeline_stats");
