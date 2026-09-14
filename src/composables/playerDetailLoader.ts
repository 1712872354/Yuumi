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
import { NEW_PLAYER_MAX_LEVEL, isIdentityCompatible } from "./identityUtils";
import { mergeMatchesWithCache } from "./gameMatchesCache";
import { computeMatchStats, computeStreak, isLikelyBotPlayer } from "./gamePlayerStats";

export interface LoadPlayerDataDeps {
  gameInfo: ReturnType<typeof useGameInfoStore>;
  store: ReturnType<typeof useLcuStore>;
  appConfig: Ref<AppConfig | null>;
  currentSummonerId: Ref<number>;
  currentSummonerPuuid: Ref<string>;
  currentQueueId: Ref<number | null>;
  onPlayerSaved: () => void;
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
      summonerId ? gameInfo.getPlayer({ summonerId }) : undefined,
      gameInfo.getPlayer({ cellId }),
    ];
    const reusable = candidates.find((e) => {
      if (!e?.info || e.loading) return false;
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
              onPlayerSaved();
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
      isHumanoid: (fallbackPlayer as PremadePlayerLike & { isHumanoid?: boolean })
        ?.isHumanoid,
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
      onPlayerSaved();
      return;
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

    try {
      let info: SummonerDisplay | null = null;
      if (playerPuuid) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v2/summoners/puuid/${playerPuuid}`,
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
            summonerId: realSummonerId || 0,
            puuid: playerPuuid || fallbackPlayer.puuid || "",
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
          matchHistoryHidden = true;
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
      if (
        !safeInfo.profileIconUrl &&
        (safeInfo.profileIconId !== undefined || safeInfo.profileIconId !== null)
      ) {
        safeInfo.profileIconUrl = `/lol-game-data/assets/v1/profile-icons/${safeInfo.profileIconId ?? 29}.jpg`;
      }

      const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
      const maxMatches = filterEnabled ? 50 : 10;

      const [rawMatches, rankedResp, masteryData] = await Promise.all([
        safeInfo.puuid
          ? fetchMatchHistorySmart(safeInfo.puuid, 0, maxMatches)
              .then((res) => {
                if (!res || res.length === 0) {
                  const lvl = safeInfo.summonerLevel ?? 0;
                  if (!(lvl > 0 && lvl < NEW_PLAYER_MAX_LEVEL)) {
                    matchHistoryHidden = true;
                  }
                }
                return res || [];
              })
              .catch((e) => {
                matchHistoryHidden = true;
                console.debug(
                  `[GameInfo] 战绩拉取失败/已隐藏 (puuid: ${safeInfo.puuid}):`,
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
          summonerId,
          summonerId === currentSummonerId.value ||
            (!!safeInfo.puuid &&
              safeInfo.puuid === currentSummonerPuuid.value),
        ),
      ]);

      const isCurrentPlayer =
        summonerId === currentSummonerId.value ||
        (!!safeInfo.puuid && safeInfo.puuid === currentSummonerPuuid.value);

      let matches: MatchDisplay[] = rawMatches;
      if (safeInfo.puuid && isCurrentPlayer) {
        matches = mergeMatchesWithCache(safeInfo.puuid, rawMatches);
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
      onPlayerSaved();
    } catch {
      const existing =
        gameInfo.getPlayer({ cellId })?.info ||
        gameInfo.getPlayer({ summonerId })?.info ||
        gameInfo.getPlayer({ puuid: playerPuuid })?.info;
      gameInfo.setPlayer(
        {
          info: existing || null,
          matches: [],
          ranked: { solo: null, flex: null },
          loading: false,
          matchHistoryHidden: true,
          championId: resolveCarryChampionId(fallbackPlayer, cellId),
        },
        { cellId, summonerId, puuid: playerPuuid },
      );
    }
  };
}
