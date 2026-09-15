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
import { inheritPlaceholderChampion } from "./identityUtils";
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
    // 对局信息页仅用实时数据：不再写入保留盘，避免与上一局串数据
  }

  // ── 对局信息页仅用实时数据：不从 localStorage 恢复上一局 ──

  // ── localStorage 写入防抖（保留盘已禁用，仅作占位避免调用方改动） ──
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
    // 实时模式：非对局阶段不展示残留内容
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
    updateCurrentQueueId,
    requestSeq,
  });

  // 监听 Watchers
  watch(isGameActive, (active) => {
    if (!active) {
      // 实时模式：离开对局清空视图，避免残留上一局
      gameflowMyTeam.value = [];
      gameflowTheirTeam.value = [];
      gameInfo.clearPlayers();
      premadeColorsMy.value = {};
      premadeColorsTheir.value = {};
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
  let lastProcessedGameId: number | null = null;
  watch(
    () => store.gameflowSession,
    (session) => {
      if (!session?.gameData) return;
      const sessionId = session.gameData.gameId ?? null;
      if (sessionId) currentGameId.value = sessionId;
      if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart") {
        // 离开对局阶段时重置，避免下一局被误判为 gameId 变化
        if (store.gamePhase === "ChampSelect" || store.gamePhase === "None") {
          lastProcessedGameId = null;
        }
        return;
      }
      const gameIdChanged =
        sessionId != null &&
        lastProcessedGameId != null &&
        sessionId !== lastProcessedGameId;
      if (gameIdChanged) {
        // 跨局：清空上一局残留，再走 processTeamData 播种当前局
        console.debug(
          `[GameInfo] gameflowSession gameId 变化: ${lastProcessedGameId} → ${sessionId}`,
        );
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        gameInfo.clearPlayers();
        premadeColorsMy.value = {};
        premadeColorsTheir.value = {};
        currentGameId.value = sessionId;
      }
      if (sessionId != null) lastProcessedGameId = sessionId;

      const { teamOne, teamTwo } = session.gameData;
      if ((teamOne && teamOne.length > 0) || (teamTwo && teamTwo.length > 0)) {
        // 队伍为空时加载；会话人数增长（如残缺会话补齐）时重处理；跨局强制重处理
        const sessionTotal = (teamOne?.length ?? 0) + (teamTwo?.length ?? 0);
        const currentTotal =
          gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
        if (
          gameIdChanged ||
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

    // 实时模式：不从 localStorage 恢复上一局

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
