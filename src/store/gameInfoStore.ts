import { defineStore } from "pinia";
import { ref } from "vue";
import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";

/**
 * GameInfo 页对局玩家数据运行时状态。
 * 由 useGamePlayerData 负责加载与写入；跨组件读取请通过本 store，避免组件私有闭包重复拉数据。
 */
export const useGameInfoStore = defineStore("gameInfo", () => {
  const loading = ref(false);
  const error = ref("");
  const currentSummonerId = ref(0);
  const currentSummonerPuuid = ref("");
  const playerData = ref<Record<string | number, PlayerData>>({});

  const champSelectTeamSnapshot = ref<PremadePlayerLike[]>([]);
  const champSelectTheirTeamSnapshot = ref<PremadePlayerLike[]>([]);

  const sessionAllyTeam = ref<PremadePlayerLike[]>([]);
  const sessionEnemyTeam = ref<PremadePlayerLike[]>([]);

  const gameflowMyTeam = ref<PremadePlayerLike[]>([]);
  const gameflowTheirTeam = ref<PremadePlayerLike[]>([]);

  const currentQueueId = ref<number | null>(null);
  const isTftMode = ref(false);
  const currentGameId = ref<number | null>(null);

  function resetPlayerDetail() {
    playerData.value = {};
  }

  function resetTeams() {
    champSelectTeamSnapshot.value = [];
    champSelectTheirTeamSnapshot.value = [];
    sessionAllyTeam.value = [];
    sessionEnemyTeam.value = [];
    gameflowMyTeam.value = [];
    gameflowTheirTeam.value = [];
  }

  function resetAll() {
    loading.value = false;
    error.value = "";
    currentSummonerId.value = 0;
    currentSummonerPuuid.value = "";
    resetPlayerDetail();
    resetTeams();
    currentQueueId.value = null;
    isTftMode.value = false;
    currentGameId.value = null;
  }

  return {
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    playerData,
    champSelectTeamSnapshot,
    champSelectTheirTeamSnapshot,
    sessionAllyTeam,
    sessionEnemyTeam,
    gameflowMyTeam,
    gameflowTheirTeam,
    currentQueueId,
    isTftMode,
    currentGameId,
    resetPlayerDetail,
    resetTeams,
    resetAll,
  };
});
