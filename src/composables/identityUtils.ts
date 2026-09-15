import type { PlayerData } from "../types/gameInfo";

/** LCU 占位空 puuid（参考 LeagueAkari EMPTY_PUUID） */
export const EMPTY_PUUID = "00000000-0000-0000-0000-000000000000";

/** 是否为可用 puuid（排除空串与 LCU 占位全 0 UUID） */
export function isValidPuuid(puuid?: string | null): boolean {
  if (!puuid) return false;
  const t = puuid.trim();
  return t.length > 0 && t !== EMPTY_PUUID;
}

// ── 无身份占位槽的前英雄 ID 继承：仅无身份占位可继承，实名异队数据严禁串用
export function inheritPlaceholderChampion(
  entry: PlayerData | undefined,
  cellId: number,
): number {
  const champ = entry?.championId ?? 0;
  if (!champ || champ <= 0) return 0;
  const ePuuid = entry?.info?.puuid ?? "";
  const eSid = entry?.info?.summonerId ?? 0;
  if (ePuuid && ePuuid !== EMPTY_PUUID) return 0;
  if (eSid && eSid !== cellId) return 0;
  return champ;
}

// ── 新号判定上限：空战绩 + 等级在此之下视为从未打过的新号，不标隐藏
export const NEW_PLAYER_MAX_LEVEL = 30;

// ── 身份门禁（canonical）：incoming 的真实身份键与条目是否冲突。
// puuid 非空即真实；summonerId 仅在非 0 且不等于 cell 槽位时视为真实
// （选人/对局切换时 cellId 会被充作 sid 兜底，不可参与比对）。
// 条目侧同理：info 缺失（loading 占位）或 sid 恰为 cell 槽位都视为无身份，不构成冲突。
// 注意这是“宽松版”（无冲突即兼容）：seed 预填充与视图层核验直接用它；
// loadPlayerData 复用已加载项时另需 finished 包装（loading 占位不可复用，占位→真实必须重拉）。
export function isIdentityCompatible(
  entry: PlayerData | undefined,
  incoming: { puuid?: string; summonerId?: number; cellId?: number },
): boolean {
  if (!entry?.info) return true;
  const cell = incoming.cellId ?? -1;
  const inPuuidRaw = (incoming.puuid || "").trim();
  const inPuuid =
    inPuuidRaw && inPuuidRaw !== EMPTY_PUUID ? inPuuidRaw : "";
  const inSid = incoming.summonerId || 0;
  const inSidReal = Boolean(inSid) && inSid !== cell;
  const ePuuidRaw = (entry.info.puuid || "").trim();
  const ePuuid =
    ePuuidRaw && ePuuidRaw !== EMPTY_PUUID ? ePuuidRaw : "";
  const eRawSid = entry.info.summonerId || 0;
  const eSid = eRawSid === cell ? 0 : eRawSid;
  if (inPuuid && ePuuid && inPuuid !== ePuuid) return false;
  if (inSidReal && eSid && inSid !== eSid) return false;
  return true;
}

/**
 * 将同一 PlayerData 以 cellId / summonerId / puuid 别名写入 playerData 表。
 * 读侧可按任一身份键查找；计数/遍历请用 uniquePlayerEntries 去重。
 */
export function setPlayerDataAliases(
  map: Record<string | number, PlayerData>,
  data: PlayerData,
  keys: { cellId?: number; summonerId?: number; puuid?: string },
): void {
  if (keys.cellId !== undefined) map[keys.cellId] = data;
  if (keys.summonerId && keys.summonerId !== keys.cellId) {
    map[keys.summonerId] = data;
  }
  if (keys.puuid) map[keys.puuid] = data;
}

/** 按对象引用去重后的玩家条目（多键别名只算一人） */
export function uniquePlayerEntries(
  map: Record<string | number, PlayerData>,
): PlayerData[] {
  return [...new Set(Object.values(map))];
}
