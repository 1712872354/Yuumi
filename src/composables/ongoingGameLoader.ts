import type { Ref } from "vue";
import { fetchOngoingGameRoster, type LiveGamePlayer } from "../api/lcu";
import { fetchMatchHistorySmart, type MatchDisplay } from "../api/lcu";
import { lcuRequest, type SummonerDisplay } from "../api/lcu";
import type { RankedStats, RankedQueueEntry } from "../types/lcu";
import type { PlayerData, PremadePlayerLike, ChampionMasteryItem } from "../types/gameInfo";
import type { useGameInfoStore } from "../store/gameInfoStore";
import { runWithConcurrency } from "../utils/runWithConcurrency";
import { computeMatchStats, computeStreak } from "./gamePlayerStats";
import { isValidPuuid } from "./identityUtils";
import { computePremadeColors } from "./usePremadeGroup";

export interface OngoingGameLoaderDeps {
  gameInfo: ReturnType<typeof useGameInfoStore>;
  gameflowMyTeam: Ref<PremadePlayerLike[]>;
  gameflowTheirTeam: Ref<PremadePlayerLike[]>;
  premadeColorsMy: Ref<Record<number, number>>;
  premadeColorsTheir: Ref<Record<number, number>>;
  currentSummonerId: Ref<number>;
  currentSummonerPuuid: Ref<string>;
  currentGameId: Ref<number | null>;
  activeTab: Ref<"my" | "their">;
  maxMatches?: number;
  /** 并发上限（对齐 LeagueAkari ongoing-game，默认 2） */
  concurrency?: number;
}

function liveToPremade(p: LiveGamePlayer, cellId: number): PremadePlayerLike {
  const name = p.gameName
    ? p.tagLine
      ? `${p.gameName}#${p.tagLine}`
      : p.gameName
    : p.summonerName || "";
  return {
    summonerId: p.summonerId || undefined,
    puuid: isValidPuuid(p.puuid) ? p.puuid : undefined,
    gameName: p.gameName || undefined,
    tagLine: p.tagLine || undefined,
    displayName: name || p.summonerName,
    summonerName: p.summonerName || undefined,
    championId: p.championId,
    profileIconId: p.profileIconId || undefined,
    cellId,
    bot: false,
    isBot: false,
  };
}

/**
 * LeagueAkari ongoing-game 同构加载：
 * roster(puuid) → 逐人并发拉 summoner / ranked / match-history(SGP) / mastery，
 * 全部实时请求，不读 localStorage、不走 TTL 缓存。
 *
 * 成功时返回 true 并写入 gameInfo + gameflow*Team。
 */
export function createOngoingGameLoader(deps: OngoingGameLoaderDeps) {
  const {
    gameInfo,
    gameflowMyTeam,
    gameflowTheirTeam,
    premadeColorsMy,
    premadeColorsTheir,
    currentSummonerId,
    currentSummonerPuuid,
    currentGameId,
    activeTab,
    maxMatches = 10,
    concurrency = 2,
  } = deps;

  async function loadOnePlayer(
    roster: LiveGamePlayer,
    cellId: number,
    reqSeq: { value: number },
    mySeq: number,
  ): Promise<void> {
    if (reqSeq.value !== mySeq) return;

    const puuid = isValidPuuid(roster.puuid) ? roster.puuid : undefined;
    const summonerId = roster.summonerId > 0 ? roster.summonerId : undefined;
    if (!puuid && !summonerId) return;

    // 1. 召唤师（v2 by puuid 优先，失败再 by sid）——实时
    let info: SummonerDisplay | null = null;
    if (puuid) {
      const resp = await lcuRequest<SummonerDisplay>(
        "GET",
        `/lol-summoner/v2/summoners/puuid/${puuid}`,
      );
      if (resp.success && resp.data) info = resp.data;
    }
    if (!info && summonerId) {
      const resp = await lcuRequest<SummonerDisplay>(
        "GET",
        `/lol-summoner/v1/summoners/${summonerId}`,
      );
      if (resp.success && resp.data) info = resp.data;
    }
    if (reqSeq.value !== mySeq) return;

    const finalPuuid = isValidPuuid(info?.puuid)
      ? info!.puuid
      : puuid;
    if (!finalPuuid) {
      // 仅有姓名的占位：标 loading 结束，避免假 hidden
      const fallbackName =
        roster.gameName || roster.summonerName || `玩家${cellId + 1}`;
      gameInfo.setPlayer(
        {
          info: {
            accountId: 0,
            summonerId: summonerId ?? 0,
            puuid: "",
            displayName: fallbackName,
            gameName: roster.gameName || fallbackName,
            tagLine: roster.tagLine || "",
            profileIconId: roster.profileIconId ?? 29,
            profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${roster.profileIconId ?? 29}.jpg`,
            summonerLevel: 0,
            percentCompleteForNextLevel: 0,
            xpSinceLastLevel: 0,
            xpUntilNextLevel: 0,
          },
          matches: [],
          ranked: { solo: null, flex: null },
          loading: false,
          matchHistoryHidden: false,
          championId: roster.championId || 0,
        },
        { cellId, summonerId, puuid: undefined },
      );
      return;
    }

    const isMe =
      (summonerId && summonerId === currentSummonerId.value) ||
      (!!finalPuuid && finalPuuid === currentSummonerPuuid.value);

    // 2. 战绩（对局中强制 SGP）+ 排位 + 熟练度 —— 全部 skipCache
    const [rawMatches, rankedResp, masteries] = await Promise.all([
      fetchMatchHistorySmart(finalPuuid, 0, maxMatches, {
        forceSgp: true,
      }).catch(() => [] as MatchDisplay[]),
      fetchRankedStatsFresh(finalPuuid),
      fetchMasteryFresh(finalPuuid, summonerId, isMe),
    ]);
    if (reqSeq.value !== mySeq) return;

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

    const stats = computeMatchStats(rawMatches);
    const streak = computeStreak(rawMatches);
    const safeInfo = info ?? {
      accountId: 0,
      summonerId: summonerId ?? 0,
      puuid: finalPuuid,
      displayName: roster.gameName || roster.summonerName || "",
      gameName: roster.gameName || "",
      tagLine: roster.tagLine || "",
      profileIconId: roster.profileIconId ?? 29,
      profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${roster.profileIconId ?? 29}.jpg`,
      summonerLevel: 0,
      percentCompleteForNextLevel: 0,
      xpSinceLastLevel: 0,
      xpUntilNextLevel: 0,
    };
    if (!safeInfo.profileIconUrl && safeInfo.profileIconId != null) {
      safeInfo.profileIconUrl = `/lol-game-data/assets/v1/profile-icons/${safeInfo.profileIconId}.jpg`;
    }

    const dataObj: PlayerData = {
      info: safeInfo,
      matches: rawMatches.slice(0, 10),
      ranked: { solo, flex },
      loading: false,
      // 实时空结果不标 hidden（对局中 LCU 常空）；真正隐私由 UI 判断
      matchHistoryHidden: false,
      championId: roster.championId || 0,
      avgKda: stats.avgKda,
      winRate: stats.winRate,
      winCount: stats.winCount,
      lossesCount: stats.lossesCount,
      masteries,
      streak,
    };
    gameInfo.setPlayer(dataObj, {
      cellId,
      summonerId: summonerId ?? info?.summonerId,
      puuid: finalPuuid,
    });
  }

  async function fetchRankedStatsFresh(puuid: string) {
    try {
      const rResp = await lcuRequest<RankedStats>(
        "GET",
        `/lol-ranked/v1/ranked-stats/${puuid}`,
      );
      if (rResp.success && rResp.data) {
        return {
          success: true as const,
          data: {
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
          },
        };
      }
      return { success: false as const };
    } catch {
      return { success: false as const };
    }
  }

  async function fetchMasteryFresh(
    puuid: string,
    summonerId: number | undefined,
    isMe: boolean,
  ): Promise<ChampionMasteryItem[]> {
    const { fetchPlayerMastery } = await import("./playerMastery");
    return fetchPlayerMastery(puuid, summonerId, isMe, { skipCache: true });
  }

  /**
   * 拉取并加载当前对局双方。返回 true 表示拿到有效 roster。
   * `mySeq` 由调用方持有（勿在此 ++requestSeq，避免外层 abortIfStale 误判过期）。
   */
  async function load(reqSeq: { value: number }, mySeq: number): Promise<boolean> {
    let roster;
    try {
      roster = await fetchOngoingGameRoster();
    } catch (e) {
      console.warn("[OngoingGame] roster 拉取失败:", e);
      return false;
    }
    if (reqSeq.value !== mySeq) return false;
    if (
      !roster ||
      (roster.myTeam.length === 0 && roster.theirTeam.length === 0)
    ) {
      console.debug("[OngoingGame] roster 为空");
      return false;
    }

    if (roster.gameId != null) currentGameId.value = roster.gameId;

    // 全量替换（实时模式，不与旧数据 merge）
    gameInfo.clearPlayers();
    const mappedMy = roster.myTeam.map((p, i) => liveToPremade(p, i));
    const mappedTheir = roster.theirTeam.map((p, i) =>
      liveToPremade(p, 5 + i),
    );
    gameflowMyTeam.value = mappedMy;
    gameflowTheirTeam.value = mappedTheir;
    premadeColorsMy.value = computePremadeColors(mappedMy);
    premadeColorsTheir.value = computePremadeColors(mappedTheir);

    // 预填 loading 占位，避免长时间空白
    for (const p of [...mappedMy, ...mappedTheir]) {
      const name =
        p.displayName || p.gameName || p.summonerName || `玩家${(p.cellId ?? 0) + 1}`;
      gameInfo.setPlayer(
        {
          info: {
            accountId: 0,
            summonerId: p.summonerId ?? 0,
            puuid: p.puuid ?? "",
            displayName: name,
            gameName: p.gameName || name,
            tagLine: p.tagLine || "",
            profileIconId: p.profileIconId ?? 29,
            profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${p.profileIconId ?? 29}.jpg`,
            summonerLevel: 0,
            percentCompleteForNextLevel: 0,
            xpSinceLastLevel: 0,
            xpUntilNextLevel: 0,
          },
          matches: [],
          ranked: { solo: null, flex: null },
          loading: true,
          championId: p.championId || 0,
        },
        { cellId: p.cellId, summonerId: p.summonerId, puuid: p.puuid },
      );
    }

    // 先加载当前可见队伍，再另一队
    const myFirst = activeTab.value === "my";
    const first = myFirst ? roster.myTeam : roster.theirTeam;
    const second = myFirst ? roster.theirTeam : roster.myTeam;
    const firstOffset = myFirst ? 0 : 5;
    const secondOffset = myFirst ? 5 : 0;

    await runWithConcurrency(
      first.map((p, i) => ({ p, cellId: firstOffset + i })),
      concurrency,
      ({ p, cellId }) => loadOnePlayer(p, cellId, reqSeq, mySeq),
    );
    void runWithConcurrency(
      second.map((p, i) => ({ p, cellId: secondOffset + i })),
      concurrency,
      ({ p, cellId }) => loadOnePlayer(p, cellId, reqSeq, mySeq),
    );

    return true;
  }

  return { load };
}
