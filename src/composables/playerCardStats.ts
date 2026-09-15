/** 对局信息卡场均衍生统计（纯函数，便于单测与复用） */

import type { MatchDisplay } from "../api/lcu";
import type { StreakInfo } from "../types/gameInfo";

function realMatches(matches: MatchDisplay[] | undefined): MatchDisplay[] {
  if (!matches || matches.length === 0) return [];
  return matches.filter((m) => !m.remake);
}

export function computeAvgCs(matches: MatchDisplay[] | undefined): number | undefined {
  const real = realMatches(matches);
  if (real.length === 0) return undefined;
  const sum = real.reduce((acc, m) => acc + (m.cs || 0), 0);
  return Math.round(sum / real.length);
}

export function computeAvgVision(
  matches: MatchDisplay[] | undefined,
): string | undefined {
  const real = realMatches(matches);
  if (real.length === 0) return undefined;
  const sum = real.reduce((acc, m) => acc + (m.visionScore || 0), 0);
  const avg = sum / real.length;
  return avg > 0 ? avg.toFixed(1) : undefined;
}

/** 伤转比：对英雄伤害 / 承受伤害 × 100 */
export function computeAvgDamageRatio(
  matches: MatchDisplay[] | undefined,
): number | undefined {
  if (!matches || matches.length === 0) return undefined;
  let dealt = 0;
  let taken = 0;
  let n = 0;
  for (const m of matches) {
    if (m.remake) continue;
    if (!m.totalDamage && !m.totalDamageTaken) continue;
    dealt += m.totalDamage || 0;
    taken += m.totalDamageTaken || 0;
    n++;
  }
  if (n === 0 || taken <= 0) return undefined;
  return Math.round((dealt / taken) * 100);
}

export function getWinRateClass(rate: number | undefined): string {
  if (rate === undefined) return "stat-dim";
  if (rate >= 53) return "stat-win";
  if (rate <= 47) return "stat-loss";
  return "stat-normal";
}

export function getKdaClass(kda: number | undefined): string {
  if (kda === undefined) return "stat-dim";
  if (kda >= 3) return "stat-win";
  if (kda < 2) return "stat-loss";
  return "stat-normal";
}

export function formatStreakBadge(
  streak: StreakInfo | null | undefined,
): { text: string; cls: string } | null {
  if (!streak || streak.count < 2) return null;
  return streak.type === "win"
    ? { text: `${streak.count}连胜`, cls: "badge-win" }
    : { text: `${streak.count}连败`, cls: "badge-loss" };
}
