import { invoke } from "@tauri-apps/api/core";

// ─── 对局上传 ───

export interface BatchUploadResult {
  successCount: number;
  failedCount: number;
  error: string | null;
}

/** 单场上传（推入后台队列） */
export const uploadSingleMatch = (gameId: number) =>
  invoke<string>("upload_single_match", { gameId });

/** 批量上传对局（直接 POST 到外部 API） */
export const batchUploadMatches = (gameIds: number[]) =>
  invoke<BatchUploadResult>("batch_upload_matches", { gameIds });
