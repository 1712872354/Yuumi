import { ref } from "vue";
import { TtlCache } from "../utils/ttlCache";
import { lcuRequest } from "../api/lcu";
import type { MatchDetail, RankedStats } from "../types/lcu";
import { formatTierCn } from "../utils/queueMeta";
import { runWithConcurrency } from "../utils/runWithConcurrency";

const GAME_DETAIL_TTL = 10 * 60 * 1000;
const RANK_TTL = 5 * 60 * 1000;
const CACHE_LIMIT = 200;

/** Search 对局详情 + 参与者段位的进程内缓存与加载 */
export function useMatchDetailCache() {
  const gameDetailCache = new TtlCache<MatchDetail>(GAME_DETAIL_TTL, CACHE_LIMIT);
  const rankCache = new TtlCache<string>(RANK_TTL, CACHE_LIMIT);

  const selectedGameId = ref<number | null>(null);
  const selectedGame = ref<MatchDetail | null>(null);
  const gameLoading = ref(false);
  const participantRanks = ref<Record<string, string>>({});

  let selectMatchRequestId = 0;

  async function loadMatchDetail(gameId: number): Promise<MatchDetail | null> {
    const cached = gameDetailCache.get(String(gameId));
    if (cached) return cached;
    const resp = await lcuRequest<MatchDetail>(
      "GET",
      `/lol-match-history/v1/games/${gameId}`,
    );
    if (resp.success && resp.data) {
      gameDetailCache.set(String(gameId), resp.data);
      return resp.data;
    }
    return null;
  }

  async function loadRanksInBackground(g: MatchDetail, requestId: number, showTier: boolean) {
    const participants = g.participants || [];
    const identities = g.participantIdentities || [];
    if (!showTier || participants.length === 0) return;

    const playerPuuids: string[] = [];
    for (const identity of identities) {
      if (identity.player?.puuid && identity.player.summonerId) {
        playerPuuids.push(identity.player.puuid);
      }
    }
    if (playerPuuids.length === 0) return;

    const rankResults: Record<string, string> = {};
    await runWithConcurrency(playerPuuids, 3, async (puuid) => {
      const cachedRank = rankCache.get(puuid);
      if (cachedRank !== null) {
        rankResults[puuid] = cachedRank;
        return;
      }
      try {
        const rResp = await lcuRequest<RankedStats>(
          "GET",
          `/lol-ranked/v1/ranked-stats/${puuid}`,
        );
        if (rResp.success && rResp.data?.queues) {
          const queues = rResp.data.queues;
          const solo = queues.find((q) => q.queueType === "RANKED_SOLO_5x5");
          const flex = queues.find((q) => q.queueType === "RANKED_FLEX_SR");
          const activeQueue = solo || flex;
          if (activeQueue && activeQueue.tier && activeQueue.tier !== "NONE") {
            const rankStr = formatTierCn(activeQueue.tier, activeQueue.rank);
            rankCache.set(puuid, rankStr);
            rankResults[puuid] = rankStr;
          }
        }
      } catch (e) {
        console.error(`拉取 PUUID 为 ${puuid} 的段位失败:`, e);
      }
    });

    if (requestId === selectMatchRequestId) {
      participantRanks.value = rankResults;
    }
  }

  async function selectMatch(gameId: number, showTier: boolean) {
    const requestId = ++selectMatchRequestId;
    selectedGameId.value = gameId;
    gameLoading.value = true;
    try {
      const g = await loadMatchDetail(gameId);
      if (!g || requestId !== selectMatchRequestId) return;
      selectedGame.value = g;
      participantRanks.value = {};
      void loadRanksInBackground(g, requestId, showTier);
    } catch (e) {
      if (requestId !== selectMatchRequestId) return;
      console.error("拉取对局详细信息失败:", e);
    } finally {
      if (requestId === selectMatchRequestId) {
        gameLoading.value = false;
      }
    }
  }

  function resetSelection() {
    selectedGame.value = null;
    selectedGameId.value = null;
    participantRanks.value = {};
  }

  return {
    selectedGameId,
    selectedGame,
    gameLoading,
    participantRanks,
    selectMatch,
    resetSelection,
  };
}
