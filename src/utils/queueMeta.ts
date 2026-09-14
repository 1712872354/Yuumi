/** 战绩队列 / 段位展示元数据（Search / Career 等页共用） */

/** 战绩页队列筛选项；`id: null` 表示「全部」 */
export const QUEUE_FILTER_OPTIONS: { id: number | null; label: string }[] = [
  { id: null, label: "全部" },
  { id: 2400, label: "海克斯大乱斗" },
  { id: 2450, label: "经典海斗" },
  { id: 450, label: "极地大乱斗" },
  { id: 430, label: "匹配模式" },
  { id: 420, label: "单双排位" },
  { id: 440, label: "灵活排位" },
];

/** 召唤师峡谷段位英文 → 官方中文名 */
export const TIER_MAP: Record<string, string> = {
  NONE: "无段位",
  IRON: "坚韧黑铁",
  BRONZE: "英勇黄铜",
  SILVER: "不屈白银",
  GOLD: "荣耀黄金",
  PLATINUM: "华贵铂金",
  EMERALD: "流光翡翠",
  DIAMOND: "璀璨钻石",
  MASTER: "超凡大师",
  GRANDMASTER: "傲世宗师",
  CHALLENGER: "最强王者",
};

/** 队列 ID → 展示名（与 Rust `get_queue_info` 对齐；i18n `gameModes.*` 优先，此处作无 i18n 场景的 fallback） */
export const QUEUE_NAME_MAP: Record<number, string> = {
  0: "自定义模式",
  400: "征召模式",
  420: "排位单双排",
  430: "匹配模式",
  440: "排位灵活组排",
  480: "快速模式",
  490: "快速模式",
  450: "极地大乱斗",
  800: "人机对战",
  810: "人机对战",
  820: "人机对战",
  830: "人机对战",
  840: "人机对战",
  850: "人机对战",
  900: "无限火力",
  1010: "随机无限火力",
  1020: "克隆模式",
  1300: "极限闪击",
  1700: "斗魂竞技场",
  1710: "斗魂竞技场",
  1810: "捉鬼模式",
  1820: "捉鬼模式",
  1830: "捉鬼模式",
  1840: "捉鬼模式",
  2400: "海克斯大乱斗",
  2450: "经典海斗",
  4300: "经典模式",
  4310: "经典模式",
};

/** 地图 ID → 展示名 */
export const MAP_NAME_MAP: Record<number, string> = {
  11: "召唤师峡谷",
  12: "嚎哭深渊",
  21: "极限闪击",
  22: "对战大厅",
  453: "经典峡谷",
};

/** 段位 + 小段（如 IV）→ 中文展示；无段位返回 tier 原文或空串 */
export function formatTierCn(tier?: string | null, division?: string | null): string {
  if (!tier || tier === "NONE") return "";
  const tierCn = TIER_MAP[tier] || tier;
  if (!division || division === "NA") return tierCn;
  return `${tierCn}${division}`;
}

/**
 * 段位展示（Career/Search 共用）：无段位时返回 `empty`（默认 "--"）。
 * `division` 传已解析好的小段（优先 rank，其次 division）。
 */
export function formatRankDisplay(
  tier?: string | null,
  division?: string | null,
  empty = "--",
): string {
  if (!tier || tier === "NONE") return empty;
  const tierCn = TIER_MAP[tier] || tier;
  if (!division || division === "NA") return tierCn;
  return `${tierCn} ${division}`;
}
