import type { PlayerData } from "../types/gameInfo";

// ── 无身份占位槽的前英雄 ID 继承：仅无身份占位可继承，实名异队数据严禁串用
export function inheritPlaceholderChampion(
  entry: PlayerData | undefined,
  cellId: number,
): number {
  const champ = entry?.championId ?? 0;
  if (!champ || champ <= 0) return 0;
  const ePuuid = entry?.info?.puuid ?? "";
  const eSid = entry?.info?.summonerId ?? 0;
  if (ePuuid) return 0;
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
  const inPuuid = (incoming.puuid || "").trim();
  const inSid = incoming.summonerId || 0;
  const inSidReal = Boolean(inSid) && inSid !== cell;
  const ePuuid = (entry.info.puuid || "").trim();
  const eRawSid = entry.info.summonerId || 0;
  const eSid = eRawSid === cell ? 0 : eRawSid;
  if (inPuuid && ePuuid && inPuuid !== ePuuid) return false;
  if (inSidReal && eSid && inSid !== eSid) return false;
  return true;
}
