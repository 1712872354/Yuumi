/** 对局信息卡段位展示（纯逻辑，与组件解耦） */

import IronMedal from "../assets/ranked-icons/iron.png";
import BronzeMedal from "../assets/ranked-icons/bronze.png";
import SilverMedal from "../assets/ranked-icons/silver.png";
import GoldMedal from "../assets/ranked-icons/gold.png";
import PlatinumMedal from "../assets/ranked-icons/platinum.png";
import EmeraldMedal from "../assets/ranked-icons/emerald.png";
import DiamondMedal from "../assets/ranked-icons/diamond.png";
import MasterMedal from "../assets/ranked-icons/master.png";
import GrandmasterMedal from "../assets/ranked-icons/grandmaster.png";
import ChallengerMedal from "../assets/ranked-icons/challenger.png";

const RANKED_MEDAL_MAP: Record<string, string> = {
  IRON: IronMedal,
  BRONZE: BronzeMedal,
  SILVER: SilverMedal,
  GOLD: GoldMedal,
  PLATINUM: PlatinumMedal,
  EMERALD: EmeraldMedal,
  DIAMOND: DiamondMedal,
  MASTER: MasterMedal,
  GRANDMASTER: GrandmasterMedal,
  CHALLENGER: ChallengerMedal,
};

export function getTierMedal(tier?: string | null): string | null {
  if (!tier) return null;
  const key = tier.toUpperCase();
  if (key === "NONE" || key === "NA") return null;
  return RANKED_MEDAL_MAP[key] || null;
}

const TIER_COLORS: Record<string, string> = {
  IRON: "#6b7280",
  BRONZE: "#b45309",
  SILVER: "#94a3b8",
  GOLD: "#d97706",
  PLATINUM: "#0d9488",
  EMERALD: "#059669",
  DIAMOND: "#3b82f6",
  MASTER: "#8b5cf6",
  GRANDMASTER: "#ef4444",
  CHALLENGER: "#f59e0b",
};

export function getTierColor(tier?: string): string {
  if (!tier) return "var(--text-dimmed, #6b7280)";
  const key = tier.toUpperCase();
  if (key === "NONE" || key === "NA") return "var(--text-dimmed, #6b7280)";
  return TIER_COLORS[key] || "var(--text-dimmed, #6b7280)";
}

const SHORT_TIER_NAMES: Record<string, string> = {
  IRON: "黑铁",
  BRONZE: "黄铜",
  SILVER: "白银",
  GOLD: "黄金",
  PLATINUM: "铂金",
  EMERALD: "翡翠",
  DIAMOND: "钻石",
  MASTER: "大师",
  GRANDMASTER: "宗师",
  CHALLENGER: "王者",
};

export interface RankEntryLike {
  tier: string;
  rank: string;
  division?: string;
  leaguePoints?: number;
}

/** 段位短文案：如「黄金IV 45」；无段位返回 unrankedLabel */
export function formatTierShort(
  entry: RankEntryLike | null,
  unrankedLabel: string,
): string {
  if (!entry || !entry.tier || entry.tier === "NA" || entry.tier === "NONE") {
    return unrankedLabel;
  }
  const tierKey = entry.tier.toUpperCase();
  const tierName = SHORT_TIER_NAMES[tierKey] || entry.tier;
  const highTier = ["MASTER", "GRANDMASTER", "CHALLENGER"].includes(tierKey);
  const lp = entry.leaguePoints !== undefined ? ` ${entry.leaguePoints}` : "";
  if (highTier) return `${tierName}${lp}`;
  const div =
    entry.rank && entry.rank !== "NA"
      ? entry.rank
      : entry.division && entry.division !== "NA"
        ? entry.division
        : "";
  if (!div) return `${tierName}${lp}`;
  return `${tierName}${div}${lp}`;
}
