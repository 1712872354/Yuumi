import type { MatchDisplay } from "../api/lcu";
import type { StreakInfo } from "../types/gameInfo";

/** 从近期对局列表计算 KDA / 胜率 / 胜负场（重开局不计入） */
export function computeMatchStats(matches: MatchDisplay[] | undefined) {
  if (!matches || matches.length === 0) {
    return {
      avgKda: undefined as number | undefined,
      winRate: undefined as number | undefined,
      winCount: undefined as number | undefined,
      lossesCount: undefined as number | undefined,
    };
  }

  let totalKills = 0;
  let totalDeaths = 0;
  let totalAssists = 0;
  let remakeCount = 0;
  let currentWinCount = 0;
  let currentLossesCount = 0;

  matches.forEach((m) => {
    if (m.remake) {
      remakeCount++;
    } else {
      totalKills += m.kills ?? 0;
      totalDeaths += m.deaths ?? 0;
      totalAssists += m.assists ?? 0;
      if (m.win) {
        currentWinCount++;
      } else {
        currentLossesCount++;
      }
    }
  });

  const winCount = currentWinCount;
  const lossesCount = currentLossesCount;
  const validMatches = matches.length - remakeCount;
  const winRate =
    validMatches > 0 ? Math.round((currentWinCount / validMatches) * 100) : 0;
  const deathsForCalc = totalDeaths === 0 ? 1 : totalDeaths;
  const avgKda = (totalKills + totalAssists) / deathsForCalc;

  return { avgKda, winRate, winCount, lossesCount };
}

/** 从近期对局列表计算连胜/连败（≥2 才展示，重开局跳过） */
export function computeStreak(matches: MatchDisplay[] | undefined): StreakInfo | null {
  if (!matches || matches.length === 0) return null;
  const validStreakMatches = matches.filter((m) => !m.remake);
  if (validStreakMatches.length === 0) return null;

  const firstWin = validStreakMatches[0].win;
  let count = 0;
  for (const m of validStreakMatches) {
    if (m.win === firstWin) {
      count++;
    } else {
      break;
    }
  }
  if (count >= 2) {
    return { type: firstWin ? "win" : "loss", count };
  }
  return null;
}

/** 本地极速识别人机/电脑玩家，避免无意义的 LCU 请求 */
export function isLikelyBotPlayer(opts: {
  fallbackBot?: boolean;
  fallbackIsBot?: boolean;
  isHumanoid?: boolean;
  botChampionId?: number | undefined;
  displayName?: string | undefined;
  summonerName?: string | undefined;
  realSummonerId: number;
  playerPuuid?: string | undefined;
}): boolean {
  return (
    Boolean(opts.fallbackBot) ||
    Boolean(opts.fallbackIsBot) ||
    Boolean(opts.isHumanoid) ||
    Boolean(opts.botChampionId) ||
    Boolean(opts.displayName?.includes("电脑")) ||
    Boolean(opts.summonerName?.includes("电脑"))
  );
}

/** 近期对局汇总：胜负 / KDA / 常用英雄（Career 页共用） */
export function computeStatsSummary(matches: MatchDisplay[]) {
  if (matches.length === 0) return null;
  let wins = 0;
  let losses = 0;
  let kills = 0;
  let deaths = 0;
  let assists = 0;
  const champMap: Record<number, { id: number; icon: string; count: number }> = {};

  for (const m of matches) {
    if (m.win) wins++;
    else losses++;
    kills += m.kills;
    deaths += m.deaths;
    assists += m.assists;

    if (!champMap[m.championId]) {
      champMap[m.championId] = {
        id: m.championId,
        icon: m.championIconUrl,
        count: 0,
      };
    }
    champMap[m.championId].count++;
  }

  const topChamps = Object.values(champMap)
    .sort((a, b) => b.count - a.count)
    .slice(0, 6);

  const kda = deaths === 0 ? "Perfect" : ((kills + assists) / deaths).toFixed(1);
  return { wins, losses, kills, deaths, assists, kda, topChamps };
}
