import { invoke } from "@tauri-apps/api/core";

/**
 * @deprecated 新代码请使用 `<LcuImage :src="path" />` / `useLcuAsset`
 * （`yuumi-asset://` 协议 URL），不要再通过 IPC 传 Base64。
 */
export const fetchLcuAsset = (path: string) =>
  invoke<string>("get_lcu_asset", { path });

/** 批量获取单个资源的单项结果 */
export interface LcuAssetItem {
  path: string;
  data_url?: string;
  error?: string;
}

/**
 * @deprecated 同 `fetchLcuAsset`，仅作兼容层保留。
 */
export const fetchLcuAssets = (paths: string[]) =>
  invoke<LcuAssetItem[]>("get_lcu_assets", { paths });
