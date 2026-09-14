import type { MatchDisplay } from "../api/lcu";
import { lazySetItem } from "../utils/lazyStorage";

const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_gf_matches_cache_${puuid}`;
/** 生涯页战绩缓存 key（本人可能先在生涯页拉过，对局信息需能回读） */
const CAREER_MATCHES_CACHE_KEY = (puuid: string) =>
  `yuumi_matches_cache_${puuid}`;
/** 单玩家本地战绩缓存上限，防止长期使用撑爆 localStorage */
const MAX_CACHED_MATCHES = 50;

/** 按 gameId 去重、时间倒序合并两份战绩，并截断到 limit */
export function mergeMatchLists(
  fresh: MatchDisplay[],
  cached: MatchDisplay[],
  limit: number,
): MatchDisplay[] {
  const seen = new Set<number>();
  return [...fresh, ...cached]
    .filter((m) => {
      if (seen.has(m.gameId)) return false;
      seen.add(m.gameId);
      return true;
    })
    .sort((a, b) => b.timeStamp - a.timeStamp)
    .slice(0, limit);
}

function readMatchList(key: string): MatchDisplay[] {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

/**
 * 合并本地缓存与最新战绩（按 gameId 去重，按时间倒序），并回写缓存。
 * 对局中 LCU 常拉不到本人战绩：缓存（含生涯页缓存）兜底，避免卡片空白。
 */
export function mergeMatchesWithCache(
  puuid: string,
  fresh: MatchDisplay[],
): MatchDisplay[] {
  const cached = mergeMatchLists(
    readMatchList(MATCHES_CACHE_KEY(puuid)),
    readMatchList(CAREER_MATCHES_CACHE_KEY(puuid)),
    MAX_CACHED_MATCHES,
  );

  const merged = mergeMatchLists(fresh, cached, MAX_CACHED_MATCHES);
  if (merged.length > 0) {
    lazySetItem(MATCHES_CACHE_KEY(puuid), merged);
  }

  return merged;
}
