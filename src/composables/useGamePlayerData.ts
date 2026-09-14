import { computed, watch, onMounted, type Ref } from "vue";
import { storeToRefs } from "pinia";
import { useLcuStore, type ChampSelectPlayer } from "../store/lcuStore";
import { useGameInfoStore } from "../store/gameInfoStore";
import {
  getGameflowPhase,
  getChampSelectSession,
  fetchCurrentSummoner,
  fetchConfig,
  fetchLiveGameTeams,
  type LiveGamePlayer,
  type AppConfig,
} from "../api/lcu";
import type {
  PlayerData,
  PremadePlayerLike,
  ChampSelectSessionLike,
} from "../types/gameInfo";
import { resolvePlayerChampionId } from "../types/gameInfo";
import type { GameflowParticipant } from "../types/lcu";
import { computePremadeColors } from "./usePremadeGroup";
import { runWithConcurrency } from "../utils/runWithConcurrency";
import { fetchPlayerMastery } from "./playerMastery";
import {
  isIdentityCompatible,
  inheritPlaceholderChampion,
} from "./identityUtils";
import { clearReserveDataFromStorage } from "./reserveData";
import {
  shouldWriteReserveData,
  writeReserveSnapshotToStorage,
  readReserveSnapshotFromStorage,
} from "./gameReserveStore";
import {
  mapGameflowParticipant,
  carryChampionFromStaleCell,
} from "./gameTeamMapping";
import {
  fetchSessionCached,
  invalidateGameflowSessionCache,
} from "./gameflowSessionCache";
import { createLoadPlayerData } from "./playerDetailLoader";

// ── 向后兼容 re-export（GameInfo.vue 等外部引用路径保持不变）
export { fetchPlayerMastery } from "./playerMastery";
export { NEW_PLAYER_MAX_LEVEL, isIdentityCompatible } from "./identityUtils";

let currentGameflowSessionRequestId = 0; // 用于防并发竞态的请求标识计数器

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
      return Object.keys(playerData.value).length > 0;
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
    inheritPlaceholderChampion(playerData.value[cellId], cellId) ||
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

  function hasPlayerIdentity(p: PremadePlayerLike): boolean {
    return Boolean(
      p.puuid || p.summonerId || p.displayName || p.gameName || p.summonerName,
    );
  }

  /**
   * 合并队伍快照：InProgress 下 LCU 可能下发同人数但无身份的“脱敏”列表，
   * 不得用它覆盖选人阶段/保留快照里已识别的玩家；人数更短时保留已有更全数据。
   */
  function mergeTeamPreservingIdentity(
    incoming: PremadePlayerLike[],
    existing: PremadePlayerLike[],
  ): PremadePlayerLike[] {
    if (incoming.length === 0) return existing;
    if (existing.length === 0) return incoming;
    if (incoming.length < existing.length) return existing;

    const incomingIdentified = incoming.filter(hasPlayerIdentity).length;
    const existingIdentified = existing.filter(hasPlayerIdentity).length;
    if (incomingIdentified === 0 && existingIdentified > 0) {
      console.debug(
        `[GameInfo] InProgress session 无身份信息，保留已有 ${existingIdentified} 名已识别玩家`,
      );
      return existing;
    }

    return incoming.map((p, idx) => {
      if (hasPlayerIdentity(p)) return p;
      const prev =
        existing.find(
          (old) =>
            (p.puuid && old.puuid === p.puuid) ||
            (p.summonerId && old.summonerId === p.summonerId) ||
            (old.cellId !== undefined && old.cellId === p.cellId && hasPlayerIdentity(old)),
        ) || existing[idx];
      if (!prev) return p;
      return {
        ...prev,
        championId: p.championId || prev.championId,
        cellId: p.cellId ?? prev.cellId,
      };
    });
  }

  function livePlayerToPremade(
    p: LiveGamePlayer,
    offset: number,
    idx: number,
  ): PremadePlayerLike {
    const name = p.gameName
      ? p.tagLine
        ? `${p.gameName}#${p.tagLine}`
        : p.gameName
      : p.summonerName || "";
    return {
      summonerId: p.summonerId || undefined,
      puuid: p.puuid || undefined,
      gameName: p.gameName || undefined,
      tagLine: p.tagLine || undefined,
      displayName: name,
      summonerName: p.summonerName || undefined,
      championId: p.championId,
      profileIconId: p.profileIconId || undefined,
      cellId: offset + idx,
      bot: false,
      isBot: false,
    };
  }

  /** InProgress 下前端 session 残缺时，用 Rust 解析的 live teams 兜底补齐对面 */
  async function tryLiveTeamsFallback() {
    if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart") {
      return;
    }
    const enemyIdentified = gameflowTheirTeam.value.filter(hasPlayerIdentity).length;
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
      // DevTools 里方便确认是否真正写入了 Pinia
      console.log("[GameInfo] live teams applied", {
        my: gameflowMyTeam.value.map((p) => p.displayName || p.puuid || p.summonerId),
        their: gameflowTheirTeam.value.map((p) => p.displayName || p.puuid || p.summonerId),
      });
      // 重新播种占位并拉取战绩
      seedInitialPlayerData(gameflowMyTeam.value);
      seedInitialPlayerData(gameflowTheirTeam.value);
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

  /** 为双方队员预填充基础占位，避免后台异步请求完成前界面出现空白 */
  function seedInitialPlayerData(players: PremadePlayerLike[]) {
    for (const p of players) {
      const key = p.cellId ?? 0;
      const isBot = Boolean(p.bot || p.isBot || p.botChampionId);
      const byPuuid = p.puuid ? playerData.value[p.puuid] : undefined;
      const bySid = p.summonerId ? playerData.value[p.summonerId] : undefined;
      const byCell = playerData.value[key];
      const incoming = { puuid: p.puuid, summonerId: p.summonerId, cellId: key };
      const existing = [byPuuid, bySid, byCell].find((e) =>
        isIdentityCompatible(e, incoming),
      );
      if (existing) {
        if (p.championId && p.championId > 0) {
          existing.championId = p.championId;
        }
        gameInfo.setPlayer(existing, {
          cellId: key,
          summonerId: p.summonerId,
          puuid: p.puuid,
        });
        continue;
      }
      if (byCell) {
        if (!p.championId || p.championId <= 0) {
          p.championId = carryChampionFromStaleCell(byCell, key, p.championId);
        }
        gameInfo.deletePlayerByCell(key);
      }
      if (!gameInfo.getPlayer({ cellId: key, summonerId: p.summonerId, puuid: p.puuid })) {
        const fallbackName =
          p.displayName ||
          p.gameName ||
          p.summonerName ||
          (isBot ? "电脑" : `玩家${key + 1}`);
        const iconId = p.profileIconId ?? 29;
        const placeholder: PlayerData = {
          info: {
            accountId: 0,
            summonerId: p.summonerId ?? 0,
            puuid: p.puuid ?? "",
            displayName: fallbackName,
            gameName: p.gameName || fallbackName,
            tagLine: p.tagLine || "",
            profileIconId: iconId,
            profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
            summonerLevel: 0,
            percentCompleteForNextLevel: 0,
            xpSinceLastLevel: 0,
            xpUntilNextLevel: 0,
          },
          matches: [],
          ranked: { solo: null, flex: null },
          loading: !isBot,
          matchHistoryHidden: isBot,
          championId: p.championId,
        };
        gameInfo.setPlayer(placeholder, {
          cellId: key,
          summonerId: p.summonerId,
          puuid: p.puuid,
        });
      }
    }
  }

  async function processTeamData(
    teamOne: GameflowParticipant[],
    teamTwo: GameflowParticipant[],
  ) {
    if (!currentSummonerId.value && !currentSummonerPuuid.value) {
      try {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      } catch {
        /* ignore */
      }
    }

    const checkMatches = (p: GameflowParticipant) => {
      const matchLocal =
        (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
        (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value);
      if (matchLocal) return true;
      // 结合选人阶段我方队员快照进行队伍识别
      return champSelectTeamSnapshot.value.some(
        (cs) =>
          (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
          (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
          (p.summonerName && cs.displayName && cs.displayName === p.summonerName) ||
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
      // 兜底：若均未匹配到当前玩家（例如自定义人机且 summonerId 延迟），非空队伍优先作为 allyTeam
      allyTeam = teamOne.length > 0 ? teamOne : teamTwo;
      enemyTeam = teamOne.length > 0 ? teamTwo : teamOne;
    }

    if (gameflowMyTeam.value && gameflowMyTeam.value.length > 0) {
      for (const p of gameflowMyTeam.value) {
        const sourceData =
          (p.cellId !== undefined ? playerData.value[p.cellId] : undefined) ||
          (p.summonerId ? playerData.value[p.summonerId] : undefined) ||
          (p.puuid ? playerData.value[p.puuid] : undefined);
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
      playerData: playerData.value,
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
    // 合并时保留身份：避免 InProgress 脱敏 session 用空身份覆盖选人/快照数据
    gameflowMyTeam.value = mergeTeamPreservingIdentity(
      mappedMy,
      gameflowMyTeam.value,
    );
    gameflowTheirTeam.value = mergeTeamPreservingIdentity(
      mappedTheir,
      gameflowTheirTeam.value,
    );
    if (mappedMy.length < gameflowMyTeam.value.length) {
      console.debug(
        `[GameInfo] 忽略残缺队伍快照: 我方新 ${mappedMy.length} 人 / 已有 ${gameflowMyTeam.value.length} 人`,
      );
    }
    if (mappedTheir.length < gameflowTheirTeam.value.length) {
      console.debug(
        `[GameInfo] 忽略残缺队伍快照: 敌方新 ${mappedTheir.length} 人 / 已有 ${gameflowTheirTeam.value.length} 人`,
      );
    }

    seedInitialPlayerData(gameflowMyTeam.value);
    seedInitialPlayerData(gameflowTheirTeam.value);

    premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
    premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);

    // 无论当前 activeTab 是哪一队，均并发启动双方队伍加载（当前可见队伍优先启动）
    // 避免 10 列视图时敌方被推迟到 background 甚至因异步延迟导致渲染空白
    const visible =
      activeTab.value === "my" ? gameflowMyTeam.value : gameflowTheirTeam.value;
    const background =
      activeTab.value === "my" ? gameflowTheirTeam.value : gameflowMyTeam.value;

    const loadTeam = (team: PremadePlayerLike[]) =>
      runWithConcurrency(team, 3, (p) =>
        loadPlayerData(p.cellId ?? 0, p.summonerId ?? 0, p.puuid, p),
      );

    // 优先启动可见队，紧随其后启动后台队，全部完成后写盘落盘
    loadTeam(visible).catch((e) =>
      console.debug("[GameInfo] 队伍数据加载异常:", e),
    );

    loadTeam(background)
      .then(() => {
        writeReserveData();
      })
      .catch((err) => {
        console.debug("[GameInfo] 队伍数据预加载失败:", err);
        writeReserveData();
      });

    // 敌方仍无有效身份时，用 Rust live teams 兜底（中途开局 / session 脱敏）
    void tryLiveTeamsFallback();
  }

  async function loadFromGameflowSession() {
    loading.value = true;
    error.value = "";

    const reqId = ++currentGameflowSessionRequestId;

    invalidateGameflowSessionCache();
    await updateCurrentQueueId();
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

    try {
      const data = await fetchSessionCached();
      if (reqId !== currentGameflowSessionRequestId) return;

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
      // InProgress 阶段 LCU 会把 gameflow 队伍精简（甚至只剩我方或单边）：
      // 当落盘有同 gameId 的完整快照，且当前队伍为空或当前队伍少于落盘人数或当前 session 少于落盘人数时，
      // 先从快照恢复整局数据，后续逻辑中的长度守卫与 identity 合并会保留更全的数据，防止敌方丢失
      const sessionTotal = t1.length + t2.length;
      const isCustomGame = data.gameData.queue?.isCustom === true;
      // 敌方列为空时优先尝试恢复保留快照（同 gameId 或近期已加载的完整对局），
      // 避免 InProgress 下 LCU 只下发我方导致对面整列空白
      const needsEnemyRestore = gameflowTheirTeam.value.length === 0;
      if (!isCustomGame && (liveGameId || needsEnemyRestore)) {
        try {
          const savedId = Number(localStorage.getItem("yuumi_last_game_id")) || 0;
          const savedTotal = Number(localStorage.getItem("yuumi_last_game_team_count")) || 0;
          const currentTotal = gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
          const sameGame = liveGameId != null && savedId === liveGameId;
          // 同局或（无 gameId 时）有完整 10 人快照且当前敌方缺失
          const canRestore =
            (sameGame &&
              (currentTotal === 0 ||
                gameflowTheirTeam.value.length === 0 ||
                (savedTotal > 0 && (sessionTotal < savedTotal || currentTotal < savedTotal)))) ||
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
          if (reqId !== currentGameflowSessionRequestId) return;
          if (
            store.gamePhase !== "InProgress" &&
            store.gamePhase !== "GameStart"
          ) {
            loading.value = false;
            return;
          }
          invalidateGameflowSessionCache();
          const retryData = await fetchSessionCached();
          if (reqId !== currentGameflowSessionRequestId) return;
          const rt = retryData?.gameData;
          if (
            rt &&
            ((rt.teamOne && rt.teamOne.length > 0) ||
              (rt.teamTwo && rt.teamTwo.length > 0))
          ) {
            return processTeamData(rt.teamOne || [], rt.teamTwo || []);
          }
          retried++;
        }
        loading.value = false;
        return;
      }

      if (t1.length === 0 || t2.length === 0) {
        // 自定义对局（人机游戏）teamTwo 可能永远为空，无需重试，直接处理。
        // 人机位于我方（myTeam 快照），敌方快照的人机位同样计入；queue.isCustom 缺失时以此兜底
        const snapshotHasBots =
          champSelectTheirTeamSnapshot.value.some((p) => p.bot || p.isBot) ||
          champSelectTeamSnapshot.value.some((p) => p.bot || p.isBot);
        if (isCustomGame || snapshotHasBots) {
          if (reqId !== currentGameflowSessionRequestId) return;
          await processTeamData(t1, t2);
          loading.value = false;
          return;
        }
        // 若其中一队为空，给它短暂重试机会（最多 3 次，每次 1 秒），若依然只有单边（如自定义单边练习），直接处理已有队伍，不阻塞
        let retried = 0;
        let currentT1 = t1;
        let currentT2 = t2;
        while (
          retried < 3 &&
          (currentT1.length === 0 || currentT2.length === 0)
        ) {
          await new Promise((r) => setTimeout(r, 1000));
          if (reqId !== currentGameflowSessionRequestId) return;
          if (
            store.gamePhase !== "InProgress" &&
            store.gamePhase !== "GameStart"
          ) {
            loading.value = false;
            return;
          }
          invalidateGameflowSessionCache();
          const retryData = await fetchSessionCached();
          if (reqId !== currentGameflowSessionRequestId) return;
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
        if (reqId !== currentGameflowSessionRequestId) return;
        await processTeamData(currentT1, currentT2);
        loading.value = false;
        return;
      }

      if (reqId !== currentGameflowSessionRequestId) return;
      await processTeamData(t1, t2);
      void tryLiveTeamsFallback();
    } catch (e) {
      if (reqId !== currentGameflowSessionRequestId) return;
      console.error("加载 gameflow session 失败:", e);
      error.value = "加载对局数据失败";
    }
    loading.value = false;
  }

  // 监听 Watchers
  watch(isGameActive, (active) => {
    if (!active) {
      // 离开游戏活跃状态（回到 Lobby / EndOfGame 等）：
      const hasPlayerData =
        gameflowMyTeam.value.length > 0 &&
        Object.keys(playerData.value).length > 0;
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
          processTeamData(teamOne || [], teamTwo || []);
        }
      }
    },
  );

  // 团队内容签名：成员 cellId + 英雄 ID（包含锁定、预选及 actions 挑选）。session 高频事件中仅倒计时变化时签名不变，跳过无效重载
  let lastSessionTeamSig = "";
  const teamSig = (team: ChampSelectPlayer[], session?: ChampSelectSessionLike | null) =>
    (team || [])
      .map((p) => {
        const champId = resolvePlayerChampionId(p, session);
        return `${p.cellId}:${champId}:${p.puuid || ""}`;
      })
      .join(",");

  watch(
    () => store.champSelectSession,
    (session) => {
      if (session && store.gamePhase === "ChampSelect") {
        const rawMyTeam = session.myTeam || [];
        const rawTheirTeam = session.theirTeam || [];

        // 队伍归属以 LCU 原生语义为准：myTeam（含自定义人机 isHumanoid）= 我方，
        // theirTeam = 敌方。人机仅标记为 bot（下游走本地占位，不请求 LCU 战绩接口），不改变归属。
        const flagBots = (list: ChampSelectPlayer[]): ChampSelectPlayer[] =>
          list.map((p) =>
            p.isHumanoid && !p.bot && !p.isBot ? { ...p, bot: true, isBot: true } : p,
          );
        // 自定义会话（敌方 cell 可能与我方共用编号空间）：敌方快照需重映射到 5+ 稳定区间隔离
        const isCustomSession =
          session.isCustomGame === true ||
          rawMyTeam.some((p) => p.isHumanoid) ||
          rawTheirTeam.some((p) => p.isHumanoid);
        const myTeam = flagBots(rawMyTeam);
        const theirTeamPlayers = flagBots(rawTheirTeam);
        const sig = teamSig(myTeam, session) + "|" + teamSig(theirTeamPlayers, session);
        if (sig === lastSessionTeamSig) return;
        lastSessionTeamSig = sig;

        // 增量更新选人阶段双方队员与已锁定英雄快照（若之前已有有效 championId，避免被过渡帧冲为 0）
        const mergeSnapshot = (
          newPlayers: ChampSelectPlayer[],
          prevSnapshot: PremadePlayerLike[],
        ): PremadePlayerLike[] => {
          return newPlayers.map((p) => {
            const detectedChampId = resolvePlayerChampionId(p, session);
            const prev = prevSnapshot.find(
              (old) =>
                (p.puuid && old.puuid === p.puuid) ||
                (p.summonerId && old.summonerId === p.summonerId) ||
                old.cellId === p.cellId,
            );
            const finalChampId =
              detectedChampId > 0
                ? detectedChampId
                : prev?.championId && prev.championId > 0
                  ? prev.championId
                  : 0;
            return {
              ...p,
              championId: finalChampId,
            };
          });
        };

        champSelectTeamSnapshot.value = mergeSnapshot(
          myTeam,
          champSelectTeamSnapshot.value,
        );
        // 增量合并敌方快照：自定义会话映射到 5+ 稳定区间；常规会话保持原 cellId 空间
        const mergedTheir = mergeSnapshot(
          theirTeamPlayers,
          champSelectTheirTeamSnapshot.value,
        );
        champSelectTheirTeamSnapshot.value = isCustomSession
          ? mergedTheir.map((p, idx) => ({ ...p, cellId: 5 + idx }))
          : mergedTheir;

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

  return {
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    playerData,
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
  };
}
