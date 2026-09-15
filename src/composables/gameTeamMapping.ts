import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";
import type { GameflowParticipant } from "../types/lcu";
import { inheritPlaceholderChampion } from "./identityUtils";

export interface MapParticipantContext {
  champSelectTeamSnapshot: PremadePlayerLike[];
  champSelectTheirTeamSnapshot: PremadePlayerLike[];
  gameflowMyTeam: PremadePlayerLike[];
  gameflowTheirTeam: PremadePlayerLike[];
  /** 按身份键取已加载玩家（优先 getPlayer，避免整表 playerData 重建） */
  lookupPlayer: (ref: {
    puuid?: string;
    summonerId?: number;
    cellId?: number;
  }) => PlayerData | undefined;
}

/**
 * 将 gameflow participant 映射为稳定 cellId 的 PremadePlayerLike。
 * 我方 offset=0（0..4），敌方 offset=5（5..9）。
 */
export function mapGameflowParticipant(
  p: GameflowParticipant,
  idx: number,
  offset: number,
  isEnemy: boolean,
  ctx: MapParticipantContext,
): PremadePlayerLike {
  const resolvedName = p.gameName
    ? p.tagLine
      ? `${p.gameName}#${p.tagLine}`
      : p.gameName
    : p.summonerName ||
      p.displayName ||
      p.botName ||
      (p.bot || p.isBot ? "电脑" : "");
  const stableCellId = offset + idx;

  let resolvedChampId = p.championId ?? p.botChampionId ?? 0;
  let profileIconId = p.profileIconId;
  const targetSnapshot = isEnemy
    ? ctx.champSelectTheirTeamSnapshot
    : ctx.champSelectTeamSnapshot;
  const snap =
    targetSnapshot.find(
      (cs) =>
        (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
        (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
        (p.summonerName && cs.displayName && cs.displayName === p.summonerName) ||
        (p.gameName && cs.gameName && cs.gameName === p.gameName) ||
        (p.displayName && cs.displayName && cs.displayName === p.displayName) ||
        (cs.cellId !== undefined && cs.cellId === idx) ||
        (cs.cellId !== undefined && cs.cellId === stableCellId),
    ) || targetSnapshot[idx];
  if (snap) {
    if (!resolvedChampId || resolvedChampId <= 0) {
      resolvedChampId = snap.championId ?? snap.championPickIntent ?? 0;
    }
    if (!profileIconId && snap.profileIconId) {
      profileIconId = snap.profileIconId;
    }
  }
  if ((!resolvedChampId || resolvedChampId <= 0) && targetSnapshot[idx]) {
    resolvedChampId =
      targetSnapshot[idx].championId ?? targetSnapshot[idx].championPickIntent ?? 0;
  }
  // InProgress 阶段 gameflow 可能不再携带 championId，从已有队伍数据继承
  if (!resolvedChampId || resolvedChampId <= 0) {
    const prevTeam = isEnemy ? ctx.gameflowTheirTeam : ctx.gameflowMyTeam;
    const prevPlayer = prevTeam.find(
      (pp) =>
        (p.puuid && pp.puuid && pp.puuid === p.puuid) ||
        (p.summonerId && pp.summonerId && pp.summonerId === p.summonerId) ||
        pp.cellId === stableCellId,
    );
    if (prevPlayer?.championId && prevPlayer.championId > 0) {
      resolvedChampId = prevPlayer.championId;
    }
  }
  if (!resolvedChampId || resolvedChampId <= 0) {
    const pd = ctx.lookupPlayer({
      puuid: p.puuid,
      summonerId: p.summonerId,
      cellId: stableCellId,
    });
    if (pd?.championId && pd.championId > 0) {
      resolvedChampId = pd.championId;
    }
  }

  // InProgress session 可能脱敏（无 puuid/sid）：从选人快照继承身份，避免只能落到「已隐藏」占位
  const rawPuuid = p.puuid && p.puuid !== "00000000-0000-0000-0000-000000000000" ? p.puuid : undefined;
  const snapPuuid =
    snap?.puuid && snap.puuid !== "00000000-0000-0000-0000-000000000000"
      ? snap.puuid
      : undefined;
  return {
    ...p,
    cellId: stableCellId,
    championId: resolvedChampId,
    summonerId: p.summonerId || snap?.summonerId,
    puuid: rawPuuid || snapPuuid,
    gameName: p.gameName || snap?.gameName,
    tagLine: p.tagLine || snap?.tagLine,
    profileIconId,
    displayName: resolvedName,
    bot: Boolean(p.bot || p.isBot),
    isBot: Boolean(p.bot || p.isBot),
  };
}

/**
 * seed 预填充时：cell 槽被异队旧数据占据时，先继承无身份占位的英雄 ID。
 */
export function carryChampionFromStaleCell(
  byCell: PlayerData | undefined,
  key: number,
  currentChampionId: number | undefined,
): number {
  if (currentChampionId && currentChampionId > 0) return currentChampionId;
  return inheritPlaceholderChampion(byCell, key);
}
