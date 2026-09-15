/** TFT 热门阵容展示辅助（纯函数） */
import type { TftMetaDeck } from "../composables/useTftMetaDecks";

export function calcDeckScore(d: TftMetaDeck): number {
  const wr = d.stat?.win_rate ?? 0;
  const pr = d.stat?.pick_rate ?? 0;
  return wr * 100 + pr * 5;
}

export interface TieredDeck {
  deck: TftMetaDeck;
  tier: string;
}

/** 按综合分排序并划 S/A/B/C/D 档（前 20% 为 S） */
export function rankDecksWithTiers(decks: TftMetaDeck[]): TieredDeck[] {
  if (!decks.length) return [];
  const sorted = [...decks].sort((a, b) => calcDeckScore(b) - calcDeckScore(a));
  const total = sorted.length;
  return sorted.map((deck, idx) => {
    const pct = idx / total;
    let tier = "D";
    if (pct < 0.2) tier = "S";
    else if (pct < 0.4) tier = "A";
    else if (pct < 0.6) tier = "B";
    else if (pct < 0.8) tier = "C";
    return { deck, tier };
  });
}

/** 从 teamCode 提取 Set 号与元数据时间，拼版本徽章文案 */
export function formatTftVersionLabel(
  decks: TftMetaDeck[] | undefined,
  gameStatDateTime?: string | undefined,
): string {
  let setStr = "";
  if (decks?.length) {
    for (const d of decks) {
      if (d.teamCode) {
        const match = d.teamCode.match(/TFTSet(\d+)/i);
        if (match) {
          setStr = `Set ${match[1]}`;
          break;
        }
      }
    }
  }
  let timeStr = "";
  if (gameStatDateTime) {
    const d = new Date(gameStatDateTime);
    if (!isNaN(d.getTime())) {
      const month = String(d.getMonth() + 1).padStart(2, "0");
      const day = String(d.getDate()).padStart(2, "0");
      const hours = String(d.getHours()).padStart(2, "0");
      const mins = String(d.getMinutes()).padStart(2, "0");
      timeStr = `${month}-${day} ${hours}:${mins}`;
    }
  }
  if (setStr && timeStr) return `OP.GG ${setStr} (${timeStr} 更新)`;
  if (setStr) return `OP.GG ${setStr}`;
  if (timeStr) return `OP.GG (${timeStr} 更新)`;
  return "OP.GG 实时版本";
}

export function formatPercent(v: number | undefined | null): string {
  if (v == null) return "--";
  return `${(v * 100).toFixed(1)}%`;
}

export function formatPlacement(v: number | undefined | null): string {
  if (v == null) return "--";
  return `#${v.toFixed(2)}`;
}

export function getDeckBadges(deck: TftMetaDeck): string[] {
  const badges: string[] = [];
  if (!deck.badge) return badges;
  for (const b of deck.badge) {
    if (b.key === "reroll" && typeof b.value === "number") {
      badges.push(`${b.value}级D牌`);
    } else if (b.key === "difficulty") {
      if (b.value === 1) badges.push("难度: 简单");
      else if (b.value === 2) badges.push("难度: 普通");
      else if (b.value === 3) badges.push("难度: 困难");
    } else if (b.key === "honey" && b.value === true) {
      badges.push("黑马上分");
    }
  }
  return badges;
}

export function getBadgeClass(badge: string): string {
  if (badge.includes("D牌")) return "badge-reroll";
  if (badge.includes("简单") || badge.includes("普通")) return "badge-easy";
  if (badge.includes("困难")) return "badge-hard";
  if (badge.includes("黑马")) return "badge-honey";
  return "badge-default";
}
