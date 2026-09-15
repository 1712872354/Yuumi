import type { Ref } from "vue";
import type { useLcuStore } from "../store/lcuStore";
import type { useGameInfoStore } from "../store/gameInfoStore";
import {
  fetchMatchHistorySmart,
  fetchPlayerFateInfo,
  lcuRequest,
  type MatchDisplay,
  type AppConfig,
  type SummonerDisplay,
} from "../api/lcu";
import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";
import type { RankedQueueEntry } from "../types/lcu";
import { fetchPlayerMastery, fetchRankedStatsCached } from "./playerMastery";
import { NEW_PLAYER_MAX_LEVEL, isIdentityCompatible, isValidPuuid } from "./identityUtils";
import { computeMatchStats, computeStreak, isLikelyBotPlayer } from "./gamePlayerStats";

export interface LoadPlayerDataDeps {
  gameInfo: ReturnType<typeof useGameInfoStore>;
  store: ReturnType<typeof useLcuStore>;
  appConfig: Ref<AppConfig | null>;
  currentSummonerId: Ref<number>;
  currentSummonerPuuid: Ref<string>;
  currentQueueId: Ref<number | null>;
  /** 玩家数据写入 store 后的可选回调（保留盘已移除，预留扩展点） */
  onPlayerSaved?: () => void;
  resolveCarryChampionId: (
    fallbackPlayer: PremadePlayerLike | undefined,
    cellId: number,
  ) => number;
}

/**
 * 单玩家详情加载：身份复用 → 人机占位 → LCU 拉取身份/战绩/段位/熟练度/宿命。
 * 写入 gameInfo 主键表（puuid / pending:cell）。
 */
export function createLoadPlayerData(deps: LoadPlayerDataDeps) {
  const {
    gameInfo,
    store,
    appConfig,
    currentSummonerId,
    currentSummonerPuuid,
    currentQueueId,
    onPlayerSaved,
    resolveCarryChampionId,
  } = deps;

  /** 同人 in-flight 去重：多入口并发时复用同一次加载 */
  const inflight = new Map<string, Promise<void>>();

  function resolveInflightKey(
    puuid: string | undefined,
    sid: number,
    cellId: number,
    entry: PlayerData | undefined,
  ): string {
    const ep = (entry?.info?.puuid || "").trim();
    if (puuid) return `puuid:${puuid}`;
    if (ep) return `puuid:${ep}`;
    if (sid) return `sid:${sid}`;
    const esid = entry?.info?.summonerId || 0;
    if (esid && esid !== cellId) return `sid:${esid}`;
    return `cell:${cellId}`;
  }

  return async function loadPlayerData(
    cellId: number,
    summonerId: number,
    playerPuuid?: string,
    fallbackPlayer?: PremadePlayerLike,
  ) {
    if (!summonerId && !playerPuuid && !fallbackPlayer) return;

    // 防止 cellId 误传为 summonerId（如 0..9 的 cellId）
    const realSummonerId = summonerId && summonerId !== cellId ? summonerId : 0;

    const incoming = { puuid: playerPuuid, summonerId: realSummonerId, cellId };
    const candidates = [
      playerPuuid ? gameInfo.getPlayer({ puuid: playerPuuid }) : undefined,
      realSummonerId ? gameInfo.getPlayer({ summonerId: realSummonerId }) : undefined,
      gameInfo.getPlayer({ cellId }),
    ];
    const reusable = candidates.find((e) => {
      if (!e?.info || e.loading) return false;
      // 空战绩且未标记「已隐藏」时不要复用（占位/对局中拉取失败的快照）
      // 新号（1~29 级）允许空战绩复用
      if (!e.matchHistoryHidden && (!e.matches || e.matches.length === 0)) {
        const lvl = e.info.summonerLevel ?? 0;
        const isNewAccount = lvl > 0 && lvl < NEW_PLAYER_MAX_LEVEL;
        if (!isNewAccount) return false;
      }
      const ePuuid = (e.info.puuid || "").trim();
      const eSid = e.info.summonerId || 0;
      if (incoming.puuid && !ePuuid) return false;
      if (incoming.summonerId && (!eSid || eSid === cellId)) return false;
      return isIdentityCompatible(e, incoming);
    });
    if (reusable) {
      gameInfo.setPlayer(reusable, {
        cellId,
        summonerId: realSummonerId,
        puuid: playerPuuid,
      });

      if (
        (!reusable.masteries || reusable.masteries.length === 0) &&
        (playerPuuid || realSummonerId)
      ) {
        const targetPuuid = playerPuuid || reusable.info?.puuid;
        const targetSid = realSummonerId || reusable.info?.summonerId;
        const isMe =
          targetSid === currentSummonerId.value ||
          (!!targetPuuid && targetPuuid === currentSummonerPuuid.value);
        fetchPlayerMastery(targetPuuid, targetSid, isMe)
          .then((m) => {
            if (m && m.length > 0) {
              reusable.masteries = m;
              onPlayerSaved?.();
            }
          })
          .catch(() => {
            /* ignore */
          });
      }
      return;
    }

    const isBotPlayer = isLikelyBotPlayer({
      fallbackBot: fallbackPlayer?.bot,
      fallbackIsBot: fallbackPlayer?.isBot,
      isHumanoid: fallbackPlayer?.isHumanoid,
      botChampionId: fallbackPlayer?.botChampionId,
      displayName: fallbackPlayer?.displayName,
      summonerName: fallbackPlayer?.summonerName,
      realSummonerId,
      playerPuuid,
    });

    if (isBotPlayer && fallbackPlayer) {
      const botName =
        fallbackPlayer.displayName ||
        fallbackPlayer.summonerName ||
        fallbackPlayer.botName ||
        fallbackPlayer.gameName ||
        `电脑${cellId + 1}`;
      const iconId = fallbackPlayer.profileIconId ?? 29;
      const botInfo: SummonerDisplay = {
        accountId: 0,
        summonerId: realSummonerId || 0,
        puuid: playerPuuid || "",
        displayName: botName,
        gameName: botName,
        tagLine: "",
        profileIconId: iconId,
        profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
        summonerLevel: 0,
        percentCompleteForNextLevel: 0,
        xpSinceLastLevel: 0,
        xpUntilNextLevel: 0,
      };
      gameInfo.setPlayer(
        {
          info: botInfo,
          matches: [],
          ranked: { solo: null, flex: null },
          loading: false,
          matchHistoryHidden: true,
          championId:
            fallbackPlayer.championId || fallbackPlayer.botChampionId || 0,
        },
        { cellId, summonerId: realSummonerId, puuid: playerPuuid },
      );
      onPlayerSaved?.();
      return;
    }

    // loading 中且已有 in-flight：等待完成后绑定，避免重复拉取
    const pendingEntry = candidates.find((e) => e?.loading);
    const inflightKey = resolveInflightKey(
      playerPuuid,
      realSummonerId,
      cellId,
      pendingEntry,
    );
    const pending = inflight.get(inflightKey);
    if (pending) {
      await pending;
      const after = [
        playerPuuid ? gameInfo.getPlayer({ puuid: playerPuuid }) : undefined,
        realSummonerId
          ? gameInfo.getPlayer({ summonerId: realSummonerId })
          : undefined,
        gameInfo.getPlayer({ cellId }),
      ].find((e) => e?.info && !e.loading);
      if (after?.info) {
        gameInfo.setPlayer(after, {
          cellId,
          summonerId: realSummonerId,
          puuid: playerPuuid || after.info.puuid || undefined,
        });
        return;
      }
      // 首载失败或未写入可用身份：继续走下面的完整加载，不要静默返回
    }

    gameInfo.setPlayer(
      {
        info: null,
        matches: [],
        ranked: { solo: null, flex: null },
        loading: true,
        championId: resolveCarryChampionId(fallbackPlayer, cellId),
      },
      { cellId, summonerId: realSummonerId, puuid: playerPuuid },
    );

    const loadTask = (async () => {
      try {
        let info: SummonerDisplay | null = null;
        // 空 puuid / LCU 占位 UUID 一律视为无身份（对齐 LeagueAkari）
        const puuidForLoad = isValidPuuid(playerPuuid) ? playerPuuid : undefined;
        if (puuidForLoad) {
          const resp = await lcuRequest<SummonerDisplay>(
            "GET",
            `/lol-summoner/v2/summoners/puuid/${puuidForLoad}`,
          );
          if (resp.success && resp.data) {
            info = resp.data;
          }
        }
        if (!info && realSummonerId) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v1/summoners/${realSummonerId}`,
        );
        if (resp.success && resp.data) {
          info = resp.data;
        }
      }
      if (!info && (fallbackPlayer?.displayName || fallbackPlayer?.gameName)) {
        const queryName = fallbackPlayer.gameName || fallbackPlayer.displayName;
        if (queryName) {
          try {
            const resp = await lcuRequest<SummonerDisplay>(
              "GET",
              `/lol-summoner/v1/summoners?name=${encodeURIComponent(queryName)}`,
            );
            if (resp.success && resp.data) {
              info = resp.data;
            }
          } catch {
            /* ignore */
          }
        }
      }

      let matchHistoryHidden = false;

      if (!info) {
        const isEnemy = store.champSelectSession?.theirTeam?.some(
          (t) =>
            (t.cellId !== undefined && t.cellId === cellId) ||
            (t.summonerId && t.summonerId === realSummonerId),
        );
        const isChampSelectEnemyWaiting =
          store.gamePhase === "ChampSelect" &&
          isEnemy &&
          !playerPuuid &&
          !realSummonerId &&
          !fallbackPlayer?.puuid &&
          !fallbackPlayer?.summonerId;

        if (fallbackPlayer && !isChampSelectEnemyWaiting) {
          const fallbackDisplayName =
            fallbackPlayer.displayName ||
            (fallbackPlayer.gameName
              ? fallbackPlayer.tagLine
                ? `${fallbackPlayer.gameName}#${fallbackPlayer.tagLine}`
                : fallbackPlayer.gameName
              : fallbackPlayer.summonerName) ||
            `玩家${cellId + 1}`;
          const iconId = fallbackPlayer.profileIconId ?? 29;
          info = {
            accountId: 0,
            summonerId: realSummonerId || fallbackPlayer.summonerId || 0,
            puuid:
              playerPuuid || fallbackPlayer.puuid || "",
            displayName: fallbackDisplayName,
            gameName: fallbackPlayer.gameName || fallbackDisplayName,
            tagLine: fallbackPlayer.tagLine || "",
            profileIconId: iconId,
            profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
            summonerLevel: 0,
            percentCompleteForNextLevel: 0,
            xpSinceLastLevel: 0,
            xpUntilNextLevel: 0,
          };
          // 先不标 hidden：若 fallback 带 puuid，下面仍会拉战绩；空结果再决定
        } else {
          gameInfo.setPlayer(
            {
              info: null,
              matches: [],
              ranked: { solo: null, flex: null },
              loading: false,
              championId: resolveCarryChampionId(fallbackPlayer, cellId),
            },
            { cellId, summonerId: realSummonerId, puuid: playerPuuid },
          );
          return;
        }
      }

      const safeInfo = info;
      if (!safeInfo.profileIconUrl && safeInfo.profileIconId != null) {
        safeInfo.profileIconUrl = `/lol-game-data/assets/v1/profile-icons/${safeInfo.profileIconId}.jpg`;
      }

      const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
      const maxMatches = filterEnabled ? 50 : 10;

      const isCurrentPlayer =
        realSummonerId === currentSummonerId.value ||
        (!!safeInfo.puuid &&
          !!currentSummonerPuuid.value &&
          safeInfo.puuid === currentSummonerPuuid.value);

      // 对局中 LCU match-history 常失败；全员（尤其敌方）强制走 SGP 合并源
      const inGamePhase =
        store.gamePhase === "InProgress" || store.gamePhase === "GameStart";
      const forceSgp = inGamePhase;

      const [rawMatches, rankedResp, masteryData] = await Promise.all([
        safeInfo.puuid
          ? fetchMatchHistorySmart(safeInfo.puuid, 0, maxMatches, {
              forceSgp,
            }).catch((e) => {
              console.debug(
                `[GameInfo] 战绩拉取失败 (puuid: ${safeInfo.puuid}):`,
                e,
              );
              return [] as MatchDisplay[];
            })
          : Promise.resolve([] as MatchDisplay[]),
        safeInfo.puuid
          ? fetchRankedStatsCached(safeInfo.puuid)
          : Promise.resolve({ success: false as const }),
        fetchPlayerMastery(
          safeInfo.puuid,
          realSummonerId,
          isCurrentPlayer,
        ),
      ]);

      let matches: MatchDisplay[] = rawMatches;
      // 对局信息页：只展示本次实时拉取结果，不合并 localStorage 战绩缓存

      // 空结果且合并缓存后仍为空 → 才视为隐藏/新号；
      // 对局中本人不因 LCU 失败误标隐藏；对局中其他人也先不标，交由 UI「暂时拉不到」态
      if (matches.length === 0) {
        const lvl = safeInfo.summonerLevel ?? 0;
        const isNewAccount = lvl > 0 && lvl < NEW_PLAYER_MAX_LEVEL;
        if (!isNewAccount && !inGamePhase) {
          matchHistoryHidden = true;
        }
      } else {
        // 拉到战绩则绝不能继续显示「已隐藏」
        matchHistoryHidden = false;
      }

      if (filterEnabled && currentQueueId.value !== null) {
        matches = matches.filter(
          (m: MatchDisplay) => m.queueId === currentQueueId.value,
        );
      }
      matches = matches.slice(0, 10);

      let solo: RankedQueueEntry | null = null;
      let flex: RankedQueueEntry | null = null;
      if (rankedResp.success && rankedResp.data?.queues) {
        solo =
          rankedResp.data.queues.find((q) => q.queueType === "RANKED_SOLO_5x5") ||
          null;
        flex =
          rankedResp.data.queues.find((q) => q.queueType === "RANKED_FLEX_SR") ||
          null;
      }

      const stats = computeMatchStats(matches);
      const streak = computeStreak(matches);

      let fateFlag: "ally" | "enemy" | null = null;
      let recentlyChampionName = "";
      if (
        currentSummonerId.value &&
        matches.length > 0 &&
        !isCurrentPlayer &&
        safeInfo.puuid
      ) {
        try {
          const lastGameId = matches[0].gameId;
          const fateInfo = await fetchPlayerFateInfo(
            lastGameId,
            safeInfo.puuid,
            currentSummonerId.value,
          );
          if (fateInfo) {
            fateFlag = fateInfo.fateFlag;
            recentlyChampionName = fateInfo.recentlyChampionName || "";
          }
        } catch (e) {
          console.debug("宿命检测失败:", e);
        }
      }

      const dataObj: PlayerData = {
        info: safeInfo,
        matches,
        ranked: { solo, flex },
        loading: false,
        matchHistoryHidden,
        championId:
          fallbackPlayer?.championId || fallbackPlayer?.botChampionId || 0,
        avgKda: stats.avgKda,
        winRate: stats.winRate,
        winCount: stats.winCount,
        lossesCount: stats.lossesCount,
        fateFlag,
        recentlyChampionName,
        masteries: masteryData,
        streak,
      };
      gameInfo.setPlayer(dataObj, {
        cellId,
        summonerId,
        puuid: safeInfo.puuid || undefined,
      });
      onPlayerSaved?.();
    } catch (e) {
      const existing =
        gameInfo.getPlayer({ cellId })?.info ||
        gameInfo.getPlayer({ summonerId })?.info ||
        gameInfo.getPlayer({ puuid: playerPuuid })?.info;
      console.debug(`[GameInfo] loadPlayerData 失败 (cell ${cellId}):`, e);
      const inGame =
        store.gamePhase === "InProgress" || store.gamePhase === "GameStart";
      gameInfo.setPlayer(
        {
          info: existing || null,
          matches: [],
          ranked: { solo: null, flex: null },
          loading: false,
          // 对局中失败不标 hidden，避免把「暂时拉不到」显示成「战绩已隐藏」
          matchHistoryHidden: !inGame,
          championId: resolveCarryChampionId(fallbackPlayer, cellId),
        },
        { cellId, summonerId, puuid: playerPuuid },
      );
    }
    })();

    inflight.set(inflightKey, loadTask);
    try {
      await loadTask;
    } finally {
      inflight.delete(inflightKey);
    }
  };
}
