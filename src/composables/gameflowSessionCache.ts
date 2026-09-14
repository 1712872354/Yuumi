import type { GameflowSession } from "../types/lcu";
import { lcuRequest } from "../api/lcu";

let cachedSession: { data: GameflowSession; timestamp: number } | null = null;
const SESSION_CACHE_TTL = 30 * 1000;

/** 清空 gameflow session 缓存（阶段切换 / 刷新时调用） */
export function invalidateGameflowSessionCache() {
  cachedSession = null;
}

/** 短期缓存 gameflow session，避免同一流程中重复请求同一端点 */
export async function fetchSessionCached(): Promise<GameflowSession | null> {
  const now = Date.now();
  if (cachedSession && now - cachedSession.timestamp < SESSION_CACHE_TTL) {
    return cachedSession.data;
  }
  try {
    const resp = await lcuRequest<GameflowSession>("GET", "/lol-gameflow/v1/session");
    if (resp.success && resp.data) {
      cachedSession = { data: resp.data, timestamp: now };
      return resp.data;
    }
  } catch {
    /* ignore */
  }
  return null;
}
