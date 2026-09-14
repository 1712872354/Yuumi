import { invoke } from "@tauri-apps/api/core";

/** 获取 LCU 静态资源（图片等），返回 data URL */
export const fetchLcuAsset = (path: string) =>
  invoke<string>("get_lcu_asset", { path });

/** 批量获取单个资源的单项结果 */
export interface LcuAssetItem {
  path: string;
  data_url?: string;
  error?: string;
}

/** 批量获取 LCU 静态资源（图片等），每个资源返回对应的 data URL（同一链路，单次 IPC 合并多个请求） */
export const fetchLcuAssets = (paths: string[]) =>
  invoke<LcuAssetItem[]>("get_lcu_assets", { paths });
