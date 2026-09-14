import type { Ref } from "vue";
import type { useLcuStore } from "../store/lcuStore";
import type { useGameInfoStore } from "../store/gameInfoStore";
import { fetchCurrentSummoner, fetchLiveGameTeams } from "../api/lcu";
import type { PremadePlayerLike } from "../types/gameInfo";
import type { GameflowParticipant } from "../types/lcu";
import { computePremadeColors } from "./usePremadeGroup";
import { runWithConcurrency } from "../utils/runWithConcurrency";
import { mapGameflowParticipant } from "./gameTeamMapping";
import {
  fetchSessionCached,
  invalidateGameflowSessionCache,
} from "./gameflowSessionCache";
import { clearReserveDataFromStorage } from "./reserveData";
import {
  hasPlayerIdentity,
  livePlayerToPremade,
  mergeTeamPreservingIdentity,
  seedInitialPlayerData,
} from "./gameflowTeamHelpers";

export interface TeamPipelineDeps {
  gameInfo: ReturnType<typeof useGameInfoStore>;
  store: ReturnType<typeof useLcuStore>;
  loading: Ref<boolean>;
  error: Ref<string>;
  currentSummonerId: Ref<number>;
  currentSummonerPuuid: Ref<string>;
  currentGameId: Ref<number | null>;
  currentQueueId: Ref<number | null>;
  isTftMode: Ref<boolean>;
  gameflowMyTeam: Ref<PremadePlayerLike[]>;
  gameflowTheirTeam: Ref<PremadePlayerLike[]>;
  champSelectTeamSnapshot: Ref<PremadePlayerLike[]>;
  champSelectTheirTeamSnapshot: Ref<PremadePlayerLike[]>;
  premadeColorsMy: Ref<Record<number, number>>;
  premadeColorsTheir: Ref<Record<number, number>>;
  activeTab: Ref<"my" | "their">;
  loadPlayerData: (
    cellId: number,
    summonerId: number,
    puuid?: string,
    fallback?: PremadePlayerLike,
  ) => Promise<void>;
  writeReserveData: () => void;
  restoreReserveDataFromLocalStorage: () => boolean;
  updateCurrentQueueId: () => Promise<void>;
  requestSeq: { value: number };
}

/**
 * 对局队伍流水线：gameflow session → 判定我/敌 → 合并身份 → 播种 → 并发拉取 → live teams 兜底。
 */
export function createGameflowTeamPipeline(deps: TeamPipelineDeps) {
  const {
    gameInfo,
    store,
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    currentGameId,
    isTftMode,
    gameflowMyTeam,
    gameflowTheirTeam,
    champSelectTeamSnapshot,
    champSelectTheirTeamSnapshot,
    premadeColorsMy,
    premadeColorsTheir,
    activeTab,
    loadPlayerData,
    writeReserveData,
    restoreReserveDataFromLocalStorage,
    updateCurrentQueueId,
    requestSeq,
  } = deps;

  function playerLookup() {
    return (ref: { puuid?: string; summonerId?: number; cellId?: number }) =>
      gameInfo.getPlayer(ref);
  }

  async function tryLiveTeamsFallback() {
    if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart") {
      return;
    }
    const enemyIdentified = gameflowTheirTeam.value.filter(hasPlayerIdentity)
      .length;
    if (enemyIdentified > 0) return;
    try {
      const live = await fetchLiveGameTeams();
      if (!live || live.theirTeam.length === 0) {
        console.debug("[GameInfo] live teams 兜底无敌方数据");
        return;
      }
      const mappedMy = live.myTeam.map((p, i) => livePlayerToPremade(p, 0, i));
      const mappedTheir = live.theirTeam.map((p, i) =>
        livePlayerToPremade(p, 5, i),
      );
      gameflowMyTeam.value = mergeTeamPreservingIdentity(
        mappedMy,
        gameflowMyTeam.value,
      );
      gameflowTheirTeam.value = mergeTeamPreservingIdentity(
        mappedTheir,
        gameflowTheirTeam.value,
      );
      if (live.gameId != null) currentGameId.value = live.gameId;
      console.debug(
        `[GameInfo] live teams 兜底: my=${gameflowMyTeam.value.length} their=${gameflowTheirTeam.value.length}`,
      );
      seedInitialPlayerData(gameInfo, gameflowMyTeam.value);
      seedInitialPlayerData(gameInfo, gameflowTheirTeam.value);
      premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
      premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
      const load = (team: PremadePlayerLike[]) =>
        runWithConcurrency(team, 3, (p) =>
          loadPlayerData(p.cellId ?? 0, p.summonerId ?? 0, p.puuid, p),
        );
      void load(gameflowMyTeam.value).catch(() => {});
      void load(gameflowTheirTeam.value)
        .then(() => writeReserveData())
        .catch(() => writeReserveData());
    } catch (e) {
      console.warn("[GameInfo] live teams 兜底失败:", e);
    }
  }

  async function processTeamData(
    teamOne: GameflowParticipant[],
    teamTwo: GameflowParticipant[],
    reqId?: number,
  ) {
    const isStale = () => reqId !== undefined && reqId !== requestSeq.value;
    if (isStale()) return;

    if (!currentSummonerId.value && !currentSummonerPuuid.value) {
      try {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      } catch {
        /* ignore */
      }
    }
    if (isStale()) return;

    const checkMatches = (p: GameflowParticipant) => {
      const matchLocal =
        (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
        (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value);
      if (matchLocal) return true;
      return champSelectTeamSnapshot.value.some(
        (cs) =>
          (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
          (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
          (p.summonerName &&
            cs.displayName &&
            cs.displayName === p.summonerName) ||
          (p.gameName && cs.gameName && cs.gameName === p.gameName),
      );
    };

    const isTeamOne = teamOne.some(checkMatches);
    const isTeamTwo = teamTwo.some(checkMatches);

    let allyTeam: GameflowParticipant[];
    let enemyTeam: GameflowParticipant[];

    if (isTeamOne) {
      allyTeam = teamOne;
      enemyTeam = teamTwo;
    } else if (isTeamTwo) {
      allyTeam = teamTwo;
      enemyTeam = teamOne;
    } else {
      allyTeam = teamOne.length > 0 ? teamOne : teamTwo;
      enemyTeam = teamOne.length > 0 ? teamTwo : teamOne;
    }

    if (gameflowMyTeam.value && gameflowMyTeam.value.length > 0) {
      for (const p of gameflowMyTeam.value) {
        const sourceData = gameInfo.getPlayer({
          cellId: p.cellId,
          summonerId: p.summonerId,
          puuid: p.puuid,
        });
        if (sourceData) {
          gameInfo.setPlayer(sourceData, {
            cellId: p.cellId,
            summonerId: p.summonerId,
            puuid: p.puuid,
          });
        }
      }
    }

    const mappingCtx = {
      champSelectTeamSnapshot: champSelectTeamSnapshot.value,
      champSelectTheirTeamSnapshot: champSelectTheirTeamSnapshot.value,
      gameflowMyTeam: gameflowMyTeam.value,
      gameflowTheirTeam: gameflowTheirTeam.value,
      lookupPlayer: playerLookup(),
    };
    const mapParticipant = (
      p: GameflowParticipant,
      idx: number,
      offset: number,
      isEnemy: boolean,
    ): PremadePlayerLike =>
      mapGameflowParticipant(p, idx, offset, isEnemy, mappingCtx);

    const mappedMy = allyTeam.map((p, idx) => mapParticipant(p, idx, 0, false));
    const mappedTheir = enemyTeam.map((p, idx) =>
      mapParticipant(p, idx, 5, true),
    );
    if (isStale()) return;

    gameflowMyTeam.value = mergeTeamPreservingIdentity(
      mappedMy,
      gameflowMyTeam.value,
    );
    gameflowTheirTeam.value = mergeTeamPreservingIdentity(
      mappedTheir,
      gameflowTheirTeam.value,
    );

    seedInitialPlayerData(gameInfo, gameflowMyTeam.value);
    seedInitialPlayerData(gameInfo, gameflowTheirTeam.value);

    premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
    premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);

    const visible =
      activeTab.value === "my" ? gameflowMyTeam.value : gameflowTheirTeam.value;
    const background =
      activeTab.value === "my" ? gameflowTheirTeam.value : gameflowMyTeam.value;

    const loadTeam = (team: PremadePlayerLike[]) =>
      runWithConcurrency(team, 3, (p) =>
        loadPlayerData(p.cellId ?? 0, p.summonerId ?? 0, p.puuid, p),
      );

    loadTeam(visible).catch((e) =>
      console.debug("[GameInfo] 队伍数据加载异常:", e),
    );

    loadTeam(background)
      .then(() => {
        if (!isStale()) writeReserveData();
      })
      .catch((err) => {
        console.debug("[GameInfo] 队伍数据预加载失败:", err);
        if (!isStale()) writeReserveData();
      });

    void tryLiveTeamsFallback();
  }

  async function loadFromGameflowSession() {
    loading.value = true;
    error.value = "";

    const reqId = ++requestSeq.value;
    /** 过期返回前必须复位 loading，避免 UI 卡在加载态 */
    const abortIfStale = () => {
      if (reqId !== requestSeq.value) {
        loading.value = false;
        return true;
      }
      return false;
    };

    invalidateGameflowSessionCache();
    await updateCurrentQueueId();
    if (abortIfStale()) return;
    if (isTftMode.value) {
      gameflowMyTeam.value = [];
      gameflowTheirTeam.value = [];
      gameInfo.clearPlayers();
      premadeColorsMy.value = {};
      premadeColorsTheir.value = {};
      clearReserveDataFromStorage();
      loading.value = false;
      return;
    }

    if (!currentSummonerId.value) {
      try {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      } catch {
        /* ignore */
      }
    }
    if (abortIfStale()) return;

    try {
      const data = await fetchSessionCached();
      if (abortIfStale()) return;

      if (!data?.gameData) {
        error.value = "无法获取对局 Session";
        loading.value = false;
        return;
      }

      const { teamOne, teamTwo } = data.gameData;
      const t1 = teamOne || [];
      const t2 = teamTwo || [];
      const liveGameId = data.gameData.gameId ?? null;
      if (liveGameId) currentGameId.value = liveGameId;
      const sessionTotal = t1.length + t2.length;
      const isCustomGame = data.gameData.queue?.isCustom === true;
      const needsEnemyRestore = gameflowTheirTeam.value.length === 0;
      if (!isCustomGame && (liveGameId || needsEnemyRestore)) {
        try {
          const savedId =
            Number(localStorage.getItem("yuumi_last_game_id")) || 0;
          const savedTotal =
            Number(localStorage.getItem("yuumi_last_game_team_count")) || 0;
          const currentTotal =
            gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
          const sameGame = liveGameId != null && savedId === liveGameId;
          const canRestore =
            (sameGame &&
              (currentTotal === 0 ||
                gameflowTheirTeam.value.length === 0 ||
                (savedTotal > 0 &&
                  (sessionTotal < savedTotal || currentTotal < savedTotal)))) ||
            (!liveGameId && needsEnemyRestore && savedTotal >= 10);
          if (canRestore) {
            const restored = restoreReserveDataFromLocalStorage();
            if (restored) {
              console.debug(
                `[GameInfo] 已从保留快照恢复队伍: my=${gameflowMyTeam.value.length} their=${gameflowTheirTeam.value.length}`,
              );
            }
          }
        } catch {
          /* ignore */
        }
      }
      if (t1.length === 0 && t2.length === 0) {
        let retried = 0;
        const maxRetries = 10;
        while (retried < maxRetries) {
          await new Promise((r) => setTimeout(r, 1000));
          if (abortIfStale()) return;
          if (
            store.gamePhase !== "InProgress" &&
            store.gamePhase !== "GameStart"
          ) {
            loading.value = false;
            return;
          }
          invalidateGameflowSessionCache();
          const retryData = await fetchSessionCached();
          if (abortIfStale()) return;
          const rt = retryData?.gameData;
          if (
            rt &&
            ((rt.teamOne && rt.teamOne.length > 0) ||
              (rt.teamTwo && rt.teamTwo.length > 0))
          ) {
            return processTeamData(rt.teamOne || [], rt.teamTwo || [], reqId);
          }
          retried++;
        }
        loading.value = false;
        return;
      }

      if (t1.length === 0 || t2.length === 0) {
        const snapshotHasBots =
          champSelectTheirTeamSnapshot.value.some((p) => p.bot || p.isBot) ||
          champSelectTeamSnapshot.value.some((p) => p.bot || p.isBot);
        if (isCustomGame || snapshotHasBots) {
          if (abortIfStale()) return;
          await processTeamData(t1, t2, reqId);
          loading.value = false;
          return;
        }
        let retried = 0;
        let currentT1 = t1;
        let currentT2 = t2;
        while (
          retried < 3 &&
          (currentT1.length === 0 || currentT2.length === 0)
        ) {
          await new Promise((r) => setTimeout(r, 1000));
          if (abortIfStale()) return;
          if (
            store.gamePhase !== "InProgress" &&
            store.gamePhase !== "GameStart"
          ) {
            loading.value = false;
            return;
          }
          invalidateGameflowSessionCache();
          const retryData = await fetchSessionCached();
          if (abortIfStale()) return;
          const rt = retryData?.gameData;
          if (rt?.teamOne?.length && rt?.teamTwo?.length) {
            currentT1 = rt.teamOne;
            currentT2 = rt.teamTwo;
            break;
          }
          if (rt?.teamOne && rt.teamOne.length > 0) currentT1 = rt.teamOne;
          if (rt?.teamTwo && rt.teamTwo.length > 0) currentT2 = rt.teamTwo;
          retried++;
        }
        if (abortIfStale()) return;
        await processTeamData(currentT1, currentT2, reqId);
        loading.value = false;
        return;
      }

      if (abortIfStale()) return;
      await processTeamData(t1, t2, reqId);
      void tryLiveTeamsFallback();
    } catch (e) {
      if (abortIfStale()) return;
      console.error("加载 gameflow session 失败:", e);
      error.value = "加载对局数据失败";
    }
    loading.value = false;
  }

  return {
    processTeamData,
    tryLiveTeamsFallback,
    loadFromGameflowSession,
    seedInitialPlayerData: (players: PremadePlayerLike[]) =>
      seedInitialPlayerData(gameInfo, players),
  };
}
