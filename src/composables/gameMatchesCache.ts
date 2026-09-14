import type { MatchDisplay } from "../api/lcu";
import { lazySetItem } from "../utils/lazyStorage";

const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_gf_matches_cache_${puuid}`;
/** 单玩家本地战绩缓存上限，防止长期使用撑爆 localStorage */
const MAX_CACHED_MATCHES = 50;

/** 合并本地缓存与最新战绩（按 gameId 去重，按时间倒序），并回写缓存 */
export function mergeMatchesWithCache(
  puuid: string,
  fresh: MatchDisplay[],
): MatchDisplay[] {
  let cached: MatchDisplay[] = [];
  try {
    const raw = localStorage.getItem(MATCHES_CACHE_KEY(puuid));
    if (raw) cached = JSON.parse(raw);
  } catch {
    /* ignore */
  }

  const seen = new Set<number>();
  const merged = [...fresh, ...cached]
    .filter((m) => {
      if (seen.has(m.gameId)) return false;
      seen.add(m.gameId);
      return true;
    })
    .sort((a, b) => b.timeStamp - a.timeStamp)
    .slice(0, MAX_CACHED_MATCHES);

  lazySetItem(MATCHES_CACHE_KEY(puuid), merged);

  return merged;
}
