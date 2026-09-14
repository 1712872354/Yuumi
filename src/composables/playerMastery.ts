import { lcuRequest, type LcuApiResponse } from "../api/lcu";
import type { ChampionMasteryItem } from "../types/gameInfo";
import type { RankedStats } from "../types/lcu";
import { TtlCache } from "../utils/ttlCache";

/** LCU 英雄熟练度原始条目（字段命名可能因接口版本略有差异） */
interface RawMasteryItem {
  championId?: number;
  champion_id?: number;
  championLevel?: number;
  masteryLevel?: number;
  level?: number;
  championPoints?: number;
  points?: number;
  score?: number;
  highestGrade?: string;
  highest_grade?: string;
  championPointsSinceLastLevel?: number;
  championPointsUntilNextLevel?: number;
  tokensEarned?: number;
}

// ── 排位 / 熟练度缓存：5 分钟 TTL，最多 100 个玩家
const rankCache = new TtlCache<RankedStats>(5 * 60 * 1000, 100);
const masteryCache = new TtlCache<ChampionMasteryItem[]>(5 * 60 * 1000, 100);

export async function fetchPlayerMastery(
  puuid?: string,
  summonerId?: number,
  isMe?: boolean,
): Promise<ChampionMasteryItem[]> {
  if (!puuid && !summonerId) return [];

  if (puuid) {
    const cached = masteryCache.get(puuid);
    if (cached && cached.length > 0) {
      return cached;
    }
  }

  // 1. 优先按 puuid 查询
  let mResp: { success: boolean; data?: RawMasteryItem[]; error?: string } = {
    success: false,
  };
  if (puuid) {
    mResp = await lcuRequest<RawMasteryItem[]>(
      "GET",
      `/lol-champion-mastery/v1/${puuid}/champion-mastery`,
    );
  }

  // 2. 若是当前玩家且按 puuid 失败（或无 puuid），降级到 local-player
  if ((!mResp.success || !mResp.data) && isMe) {
    mResp = await lcuRequest<RawMasteryItem[]>(
      "GET",
      "/lol-champion-mastery/v1/local-player/champion-mastery",
    );
  }

  // 3. 如果仍未成功，尝试按 summonerId 查询
  if ((!mResp.success || !mResp.data) && summonerId) {
    mResp = await lcuRequest<RawMasteryItem[]>(
      "GET",
      `/lol-champion-mastery/v1/summoners/${summonerId}/champion-mastery`,
    );
  }

  if (mResp.success && Array.isArray(mResp.data)) {
    const normalized: ChampionMasteryItem[] = mResp.data.map((item) => ({
      championId: Number(item.championId ?? item.champion_id ?? 0),
      championLevel: Number(item.championLevel ?? item.masteryLevel ?? item.level ?? 0),
      championPoints: Number(item.championPoints ?? item.points ?? item.score ?? 0),
      highestGrade: item.highestGrade ?? item.highest_grade,
      championPointsSinceLastLevel: item.championPointsSinceLastLevel,
      championPointsUntilNextLevel: item.championPointsUntilNextLevel,
      tokensEarned: item.tokensEarned,
    }));
    if (puuid) {
      masteryCache.set(puuid, normalized);
    }
    return normalized;
  }

  return [];
}

/** 按 puuid 拉取排位数据（带 5 分钟缓存）。统一把 division 归一到 rank。 */
export async function fetchRankedStatsCached(
  puuid: string,
): Promise<LcuApiResponse<RankedStats>> {
  const cached = rankCache.get(puuid);
  if (cached) {
    return { success: true, data: cached };
  }
  try {
    const rResp = await lcuRequest<RankedStats>(
      "GET",
      `/lol-ranked/v1/ranked-stats/${puuid}`,
    );
    if (rResp.success && rResp.data) {
      const data: RankedStats = {
        ...rResp.data,
        queues: (rResp.data.queues || []).map((q) => {
          const rank =
            q.rank && q.rank !== "NA" && q.rank !== ""
              ? q.rank
              : q.division && q.division !== "NA"
                ? q.division
                : q.rank;
          return { ...q, rank };
        }),
      };
      rankCache.set(puuid, data);
      return { success: true, data };
    }
    return rResp;
  } catch {
    return { success: false };
  }
}
