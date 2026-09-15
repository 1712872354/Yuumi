/** 战绩队列 / 段位展示元数据（与 Rust 共用 `src/shared/queue_meta.json` 单一数据源） */

import queueMetaRaw from "../shared/queue_meta.json";

export interface QueueMetaEntry {
  name: string;
  map: string;
}

interface QueueMetaFile {
  tftQueueIds: number[];
  queues: Record<string, QueueMetaEntry>;
  maps: Record<string, string>;
}

const queueMeta = queueMetaRaw as QueueMetaFile;

/** 云顶队列 ID（与 Rust `is_tft_queue` 同源） */
export const TFT_QUEUE_IDS: readonly number[] = queueMeta.tftQueueIds;

export function isTftQueue(queueId: number): boolean {
  return TFT_QUEUE_IDS.includes(queueId);
}

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

/** 队列 ID → 展示名（与 Rust `get_queue_info` 同源；i18n `gameModes.*` 优先，此处作无 i18n 场景的 fallback） */
export const QUEUE_NAME_MAP: Record<number, string> = Object.fromEntries(
  Object.entries(queueMeta.queues).map(([id, meta]) => [Number(id), meta.name]),
);

/** 战绩页队列筛选项；`id: null` 表示「全部」。label 与 QUEUE_NAME_MAP 保持一致 */
export const QUEUE_FILTER_OPTIONS: { id: number | null; label: string }[] = (
  [null, 2400, 2450, 450, 430, 420, 440] as const
).map((id) => ({
  id,
  label: id === null ? "全部" : (QUEUE_NAME_MAP[id] ?? String(id)),
}));

/** 地图 ID → 展示名（与 Rust 共用） */
export const MAP_NAME_MAP: Record<number, string> = Object.fromEntries(
  Object.entries(queueMeta.maps).map(([id, name]) => [Number(id), name]),
);

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
