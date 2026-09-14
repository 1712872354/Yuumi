import { computed, watch, onMounted, type Ref } from "vue";
import { storeToRefs } from "pinia";
import { useLcuStore, type ChampSelectPlayer } from "../store/lcuStore";
import { useGameInfoStore } from "../store/gameInfoStore";
import {
  getGameflowPhase,
  getChampSelectSession,
  fetchCurrentSummoner,
  fetchConfig,
  type AppConfig,
} from "../api/lcu";
import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";
import { computePremadeColors } from "./usePremadeGroup";
import { runWithConcurrency } from "../utils/runWithConcurrency";
import { fetchPlayerMastery } from "./playerMastery";
import { inheritPlaceholderChampion } from "./identityUtils";
import {
  shouldWriteReserveData,
  writeReserveSnapshotToStorage,
  readReserveSnapshotFromStorage,
} from "./gameReserveStore";
import {
  fetchSessionCached,
  invalidateGameflowSessionCache,
} from "./gameflowSessionCache";
import { createLoadPlayerData } from "./playerDetailLoader";
import { createGameflowTeamPipeline } from "./gameflowTeamPipeline";
import {
  buildTeamSig as teamSig,
  flagBots,
  isCustomChampSelectSession,
  mergeChampSelectSnapshot,
  remapTheirSnapshotForCustom,
} from "./champSelectSnapshot";

// 向后兼容 re-export（GameInfo.vue 等外部引用路径保持不变）
export { fetchPlayerMastery } from "./playerMastery";
export { NEW_PLAYER_MAX_LEVEL, isIdentityCompatible } from "./identityUtils";

/**
 * GameInfo 对局数据源（阶段 → 状态优先级）
 *
 * | 阶段 | 队伍列表 myTeam/theirTeam | 玩家详情 playerData |
 * |------|---------------------------|---------------------|
 * | ChampSelect | gameflow*Team → champSelectSession.myTeam/theirTeam → champSelect*Snapshot | 逐人 loadPlayerData |
 * | GameStart / InProgress | gameflow*Team（session 合并身份后） | 同上 + live teams 兜底补齐敌方 |
 * | 非对局（保留盘开启） | localStorage 恢复的 gameflow*Team | restorePlayers 后的主键表 |
 *
 * 内部主键：puuid（无身份 pending:{cellId}）；cellId/sid 仅别名。
 * 视图读取请用 findPlayerData / store.getPlayer，勿再手写多键扫描。
 */
export function useGamePlayerData(
  appConfig: Ref<AppConfig | null>,
  premadeColorsMy: Ref<Record<number, number>>,
  premadeColorsTheir: Ref<Record<number, number>>,
  activeTab: Ref<"my" | "their">,
) {
  const store = useLcuStore();
  const gameInfo = useGameInfoStore();
  const {
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    champSelectTeamSnapshot,
    champSelectTheirTeamSnapshot,
    sessionAllyTeam,
    sessionEnemyTeam,
    gameflowMyTeam,
    gameflowTheirTeam,
    currentQueueId,
    isTftMode,
    currentGameId,
  } = storeToRefs(gameInfo);

  function writeReserveData() {
    // 内部主键表已按 puuid/pending 去重，无需再 Set
    const loadedCount = gameInfo.uniquePlayerList().filter((d) => d.info !== null)
      .length;
    if (
      !shouldWriteReserveData({
        loadedCount,
        gamePhase: store.gamePhase,
        currentGameId: currentGameId.value,
        myTeamLength: gameflowMyTeam.value.length,
        theirTeamLength: gameflowTheirTeam.value.length,
      })
    ) {
      return;
    }
    writeReserveSnapshotToStorage({
      playerData: gameInfo.players,
      myTeam: gameflowMyTeam.value,
      theirTeam: gameflowTheirTeam.value,
      premadeColorsMy: premadeColorsMy.value,
      premadeColorsTheir: premadeColorsTheir.value,
      loadedCount,
      gameId: currentGameId.value,
    });
  }

  // ── 从 localStorage 恢复保留数据（有数据即恢复，直到新对局开始）
  let isBackfillingMasteries = false;
  function backfillMissingMasteries() {
    if (isBackfillingMasteries) return;
    const entries = gameInfo.uniquePlayerList();
    const needBackfill = entries.filter(
      (e) =>
        e &&
        e.info &&
        !e.loading &&
        (!e.masteries || e.masteries.length === 0) &&
        (e.info.puuid || e.info.summonerId),
    );
    if (needBackfill.length === 0) return;

    isBackfillingMasteries = true;
    // 异步排队后台拉取，每人间隔 100ms，避免突发并发
    Promise.allSettled(
      needBackfill.map((p, idx) =>
        new Promise<void>((resolve) => {
          setTimeout(async () => {
            try {
              const puuid = p.info?.puuid;
              const sid = p.info?.summonerId;
              const isMe =
                sid === currentSummonerId.value ||
                (!!puuid && puuid === currentSummonerPuuid.value);
              const m = await fetchPlayerMastery(puuid, sid, isMe);
              if (m && m.length > 0) {
                p.masteries = m;
              }
            } catch {
              /* ignore */
            } finally {
              resolve();
            }
          }, idx * 100);
        }),
      ),
    ).finally(() => {
      isBackfillingMasteries = false;
      debouncedSavePlayerData();
    });
  }

  function restoreReserveDataFromLocalStorage(): boolean {
    try {
      const snap = readReserveSnapshotFromStorage();
      let hasRestored = false;
      if (snap.myTeam) {
        gameflowMyTeam.value = snap.myTeam;
        hasRestored = true;
      }
      if (snap.theirTeam) {
        gameflowTheirTeam.value = snap.theirTeam;
        hasRestored = true;
      }
      if (snap.playerData) {
        gameInfo.restorePlayers(snap.playerData);
        hasRestored = true;
        // 异步检查并补齐缺少熟练度的玩家（例如旧版本保留的数据）
        backfillMissingMasteries();
      }
      if (snap.premadeColorsMy) {
        premadeColorsMy.value = snap.premadeColorsMy;
      } else if (gameflowMyTeam.value.length > 0) {
        premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
      }
      if (snap.premadeColorsTheir) {
        premadeColorsTheir.value = snap.premadeColorsTheir;
      } else if (gameflowTheirTeam.value.length > 0) {
        premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
      }
      return hasRestored;
    } catch {
      return false;
    }
  }

  // ── localStorage 写入防抖
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedSavePlayerData() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      writeReserveData();
    }, 500);
  }

  const myTeam = computed(() => {
    if (isGameActive.value) {
      if (gameflowMyTeam.value.length > 0) return gameflowMyTeam.value;
      if (store.champSelectSession?.myTeam && store.champSelectSession.myTeam.length > 0) {
        // LCU 原生归属：myTeam（含自定义人机）即我方，直接返回（人机由下游按 bot 处理）
        return store.champSelectSession.myTeam;
      }
      if (champSelectTeamSnapshot.value.length > 0) {
        return champSelectTeamSnapshot.value;
      }
    }
    return gameflowMyTeam.value;
  });

  const theirTeam = computed(() => {
    if (isGameActive.value) {
      if (gameflowTheirTeam.value.length > 0) return gameflowTheirTeam.value;
      if (store.champSelectSession?.theirTeam) {
        // LCU 原生归属：theirTeam 即敌方；空占位（无任何身份）丢弃，避免幽灵列
        const real = (store.champSelectSession.theirTeam || []).filter(
          (p) => p.isHumanoid || p.puuid || p.summonerId || p.championId,
        );
        if (real.length > 0) return real;
      }
      if (champSelectTheirTeamSnapshot.value.length > 0) {
        return champSelectTheirTeamSnapshot.value;
      }
    }
    return gameflowTheirTeam.value;
  });

  const currentTeam = computed(() =>
    activeTab.value === "my" ? myTeam.value : theirTeam.value,
  );

  const isGameActive = computed(
    () =>
      store.gamePhase === "ChampSelect" ||
      store.gamePhase === "GameStart" ||
      store.gamePhase === "InProgress",
  );

  const shouldShowContent = computed(() => {
    if (isTftMode.value) return false;
    if (isGameActive.value) return true;
    if (appConfig.value?.Functions?.EnableReserveGameinfo) {
      return gameInfo.uniquePlayerList().length > 0;
    }
    return false;
  });

  async function updateCurrentQueueId() {
    try {
      const data = await fetchSessionCached();
      if (data?.gameData?.queue?.id !== undefined) {
        currentQueueId.value = data.gameData.queue.id;
        const qId = currentQueueId.value;
        const gameMode = data.gameData.queue.gameMode;
        if (gameMode === "TFT" || (qId !== null && qId >= 1090 && qId <= 1200)) {
          isTftMode.value = true;
        } else {
          isTftMode.value = false;
        }
      } else {
        currentQueueId.value = null;
        isTftMode.value = false;
      }
    } catch {
      currentQueueId.value = null;
      isTftMode.value = false;
    }
  }

  async function fetchPremadeColors() {
    try {
      if (!currentSummonerId.value && !currentSummonerPuuid.value) {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      }
      const data = await fetchSessionCached();
      if (data?.gameData) {
        const { teamOne, teamTwo } = data.gameData;
        const t1 = teamOne || [];
        const t2 = teamTwo || [];
        if (t1.length > 0 || t2.length > 0) {
          const isTeamOne = t1.some(
            (p) =>
              (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
              (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value),
          );
          const ally = isTeamOne || t2.length === 0 ? t1 : t2;
          const enemy = isTeamOne || t2.length === 0 ? t2 : t1;
          sessionAllyTeam.value = ally;
          sessionEnemyTeam.value = enemy;
          premadeColorsMy.value = computePremadeColors(ally);
          premadeColorsTheir.value = computePremadeColors(enemy);
          return;
        }
      }
      // 备选降级：若 gameflow 暂无组队数据，但选人 session 已有 teamParticipantId
      if (
        Object.keys(premadeColorsMy.value).length === 0 &&
        store.champSelectSession?.myTeam?.some(
          (p: ChampSelectPlayer) =>
            p.teamParticipantId !== undefined || p.partyId !== undefined,
        )
      ) {
        premadeColorsMy.value = computePremadeColors(
          store.champSelectSession.myTeam,
        );
      }
      if (
        Object.keys(premadeColorsTheir.value).length === 0 &&
        store.champSelectSession?.theirTeam?.some(
          (p: ChampSelectPlayer) =>
            p.teamParticipantId !== undefined || p.partyId !== undefined,
        )
      ) {
        premadeColorsTheir.value = computePremadeColors(
          store.champSelectSession.theirTeam,
        );
      }
    } catch {
      /* ignore */
    }
  }

  async function refreshState() {
    loading.value = true;
    try {
      const phaseResp = await getGameflowPhase();
      if (phaseResp.success && phaseResp.data) {
        store.setGamePhase(phaseResp.data);
        if (phaseResp.data === "InProgress" || phaseResp.data === "GameStart") {
          loadFromGameflowSession();
        }
      }
    } catch {
      /* ignore */
    }
    try {
      const sessionResp = await getChampSelectSession();
      if (sessionResp.success && sessionResp.data) {
        store.setChampSelectSession(sessionResp.data);
      }
    } catch {
      /* ignore */
    }
    loading.value = false;
  }

  // 拉取窗口 / 拉取失败 / 异常兜底三处的英雄保留：fallback 自带优先，
  // 否则仅继承无身份占位的（实名异队数据严禁串用），保证加载中头像不断档
  const resolveCarryChampionId = (
    fallbackPlayer: PremadePlayerLike | undefined,
    cellId: number,
  ): number =>
    fallbackPlayer?.championId ||
    fallbackPlayer?.botChampionId ||
    inheritPlaceholderChampion(gameInfo.getPlayer({ cellId }), cellId) ||
    0;

  const loadPlayerData = createLoadPlayerData({
    gameInfo,
    store,
    appConfig,
    currentSummonerId,
    currentSummonerPuuid,
    currentQueueId,
    onPlayerSaved: debouncedSavePlayerData,
    resolveCarryChampionId,
  });

  async function loadAllPlayers() {
    const my = myTeam.value;
    const their = theirTeam.value;
    if (my.length === 0 && their.length === 0) return;
    await updateCurrentQueueId();

    const filterValidPlayers = (team: PremadePlayerLike[], isEnemy: boolean) => {
      if (store.gamePhase === "ChampSelect" && isEnemy) {
        // 选人阶段敌方队伍若无有效身份且非人机，不请求
        //（敌方人机含原生 isHumanoid 标记，同样放行走 bot 本地占位）
        return team.filter(
          (p) =>
            Boolean(
              p.puuid ||
              p.summonerId ||
              p.bot ||
              p.isBot ||
              p.botChampionId ||
              (p as PremadePlayerLike & { isHumanoid?: boolean }).isHumanoid ||
              p.displayName ||
              p.summonerName,
            ),
        );
      }
      return team;
    };

    const isMyVisible = activeTab.value === "my";
    // 先加载当前可见队伍，再后台加载另一队，避免请求风暴
    const visible = filterValidPlayers(isMyVisible ? my : their, !isMyVisible);
    const background = filterValidPlayers(isMyVisible ? their : my, isMyVisible);

    await runWithConcurrency(visible, 3, (p) => {
      const cid = p.cellId ?? p.summonerId ?? 0;
      const sid = p.summonerId ?? 0;
      if (cid !== undefined && (sid || p.puuid || p.displayName || p.summonerName)) {
        return loadPlayerData(cid, sid, p.puuid, p);
      }
      return Promise.resolve();
    });
    void runWithConcurrency(background, 3, (p) => {
      const cid = p.cellId ?? p.summonerId ?? 0;
      const sid = p.summonerId ?? 0;
      if (cid !== undefined && (sid || p.puuid || p.displayName || p.summonerName)) {
        return loadPlayerData(cid, sid, p.puuid, p);
      }
      return Promise.resolve();
    })
      .then(() => {
        writeReserveData();
      })
      .catch((err) => {
        console.debug("[GameInfo] 后台队伍数据预加载失败:", err);
        writeReserveData();
      });
  }

  const requestSeq = { value: 0 };
  const { loadFromGameflowSession, processTeamData } = createGameflowTeamPipeline({
    gameInfo,
    store,
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    currentGameId,
    currentQueueId,
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
  });

  // 监听 Watchers
  watch(isGameActive, (active) => {
    if (!active) {
      // 离开游戏活跃状态（回到 Lobby / EndOfGame 等）：
      const hasPlayerData =
        gameflowMyTeam.value.length > 0 &&
        gameInfo.uniquePlayerList().length > 0;
      if (hasPlayerData) {
        // 内存中已有刚打完的对局，确保落盘
        writeReserveData();
      } else {
        // 否则（如中途启动或选人秒退离开），尝试从 localStorage 恢复上一次对局快照
        restoreReserveDataFromLocalStorage();
      }
    } else {
      // 刚进入选人阶段时，清空当前内存视图以展示当前选人
      if (store.gamePhase === "ChampSelect") {
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        gameInfo.clearPlayers();
      }
    }
  });

  watch(
    () => store.gamePhase,
    (phase: string) => {
      if (phase !== "InProgress" && phase !== "GameStart") {
        isTftMode.value = false;
      }
      if (phase === "ChampSelect") {
        currentGameId.value = null;
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        gameInfo.clearPlayers();
        premadeColorsMy.value = {};
        premadeColorsTheir.value = {};
        // 选人阶段不立即清空 localStorage，避免秒退导致已有完整对局丢失
        refreshState();
      }
      if (phase === "InProgress" || phase === "GameStart") {
        // 从选人阶段进入载入或游戏时，清空 session 缓存
        invalidateGameflowSessionCache();
        loadFromGameflowSession();
      }
    },
    { immediate: true },
  );

  // 监听 WebSocket 推送的 gameflowSession 变化，对局就绪即刻自动解析
  watch(
    () => store.gameflowSession,
    (session) => {
      if (!session?.gameData) return;
      if (session.gameData.gameId) currentGameId.value = session.gameData.gameId;
      if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart") return;
      const { teamOne, teamTwo } = session.gameData;
      if ((teamOne && teamOne.length > 0) || (teamTwo && teamTwo.length > 0)) {
        // 队伍为空时加载；会话人数增长（如残缺会话补齐）时重处理，补回缺失的列
        const sessionTotal = (teamOne?.length ?? 0) + (teamTwo?.length ?? 0);
        const currentTotal =
          gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
        if (
          gameflowMyTeam.value.length === 0 ||
          gameflowTheirTeam.value.length === 0 ||
          (currentTotal > 0 && sessionTotal > currentTotal)
        ) {
          // 不要 ++requestSeq：那会取消进行中的 loadFromGameflowSession，
          // 导致整场对局详情半途中断（loading 卡住 / 玩家数据不全）
          processTeamData(teamOne || [], teamTwo || []);
        }
      }
    },
  );

  // 团队内容签名：成员 cellId + 英雄 ID。session 高频事件中仅倒计时变化时签名不变，跳过无效重载
  let lastSessionTeamSig = "";

  watch(
    () => store.champSelectSession,
    (session) => {
      if (session && store.gamePhase === "ChampSelect") {
        const rawMyTeam = session.myTeam || [];
        const rawTheirTeam = session.theirTeam || [];

        // 队伍归属以 LCU 原生语义为准：myTeam（含自定义人机 isHumanoid）= 我方，
        // theirTeam = 敌方。人机仅标记为 bot（下游走本地占位，不请求 LCU 战绩接口），不改变归属。
        const isCustomSession = isCustomChampSelectSession(
          session,
          rawMyTeam,
          rawTheirTeam,
        );
        const myTeam = flagBots(rawMyTeam);
        const theirTeamPlayers = flagBots(rawTheirTeam);
        const sig =
          teamSig(myTeam, session) + "|" + teamSig(theirTeamPlayers, session);
        if (sig === lastSessionTeamSig) return;
        lastSessionTeamSig = sig;

        // 增量更新选人阶段双方队员与已锁定英雄快照
        champSelectTeamSnapshot.value = mergeChampSelectSnapshot(
          myTeam,
          champSelectTeamSnapshot.value,
          session,
        );
        const mergedTheir = mergeChampSelectSnapshot(
          theirTeamPlayers,
          champSelectTheirTeamSnapshot.value,
          session,
        );
        champSelectTheirTeamSnapshot.value = remapTheirSnapshotForCustom(
          mergedTheir,
          isCustomSession,
        );

        loading.value = false;
        error.value = "";
        gameflowMyTeam.value = champSelectTeamSnapshot.value;
        gameflowTheirTeam.value = champSelectTheirTeamSnapshot.value;
        loadAllPlayers();
        fetchPremadeColors();
      }
    },
  );

  watch(activeTab, () => loadAllPlayers());

  watch(
    () => store.currentPage,
    (newPage) => {
      if (newPage === "gameinfo") {
        if (store.gamePhase === "InProgress" || store.gamePhase === "GameStart") {
          loadFromGameflowSession();
        } else if (store.gamePhase === "ChampSelect") {
          loadAllPlayers();
        } else {
          refreshState();
        }
      }
    },
  );

  onMounted(async () => {
    if (!appConfig.value) {
      try {
        appConfig.value = await fetchConfig();
      } catch {
        /* ignore */
      }
    }

    if (appConfig.value?.Functions?.EnableReserveGameinfo) {
      if (restoreReserveDataFromLocalStorage()) {
        if (Object.keys(premadeColorsMy.value).length === 0 && gameflowMyTeam.value.length > 0)
          premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
        if (Object.keys(premadeColorsTheir.value).length === 0 && gameflowTheirTeam.value.length > 0)
          premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
      }
    }

    try {
      const s = await fetchCurrentSummoner();
      if (s?.summonerId) currentSummonerId.value = s.summonerId;
      if (s?.puuid) currentSummonerPuuid.value = s.puuid;
    } catch {
      /* ignore */
    }

    refreshState();
  });

  /**
   * 按队伍成员身份查找已加载的 PlayerData。
   * 优先 puuid/sid/cell（store 别名表），再退化到槽位与显示名匹配。
   */
  function findPlayerData(
    p: PremadePlayerLike,
    idx: number,
    side: "ally" | "enemy",
  ): PlayerData | undefined {
    const incoming = {
      puuid: p.puuid,
      summonerId: p.summonerId,
      cellId: p.cellId,
    };
    const hit = gameInfo.getPlayer(incoming);
    if (hit) return hit;

    const offset = side === "enemy" ? 5 : 0;
    const bySlot = gameInfo.getPlayer({ cellId: offset + idx });
    if (bySlot) return bySlot;

    const pName = p.displayName || p.gameName || p.summonerName;
    if (!pName) return undefined;
    return gameInfo
      .uniquePlayerList()
      .find((d) => {
        const dName = d.info?.displayName || d.info?.gameName;
        return Boolean(dName && dName === pName);
      });
  }

  return {
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    sessionAllyTeam,
    sessionEnemyTeam,
    gameflowMyTeam,
    gameflowTheirTeam,
    currentQueueId,
    isTftMode,
    myTeam,
    theirTeam,
    currentTeam,
    isGameActive,
    shouldShowContent,
    refreshState,
    loadAllPlayers,
    loadFromGameflowSession,
    findPlayerData,
  };
}
