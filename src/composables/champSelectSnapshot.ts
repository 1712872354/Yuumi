import type { ChampSelectPlayer } from "../store/lcuStore";
import type {
  ChampSelectSessionLike,
  PremadePlayerLike,
} from "../types/gameInfo";
import { resolvePlayerChampionId } from "../types/gameInfo";

/** 团队内容签名：成员 cellId + 英雄 ID（含锁定/预选/actions）。倒计时变化时签名不变可跳过重载 */
export function buildTeamSig(
  team: ChampSelectPlayer[],
  session?: ChampSelectSessionLike | null,
): string {
  return (team || [])
    .map((p) => {
      const champId = resolvePlayerChampionId(p, session);
      return `${p.cellId}:${champId}:${p.puuid || ""}`;
    })
    .join(",");
}

/** 人机（isHumanoid）仅标记 bot，不改变 LCU 队伍归属 */
export function flagBots(list: ChampSelectPlayer[]): ChampSelectPlayer[] {
  return list.map((p) =>
    p.isHumanoid && !p.bot && !p.isBot ? { ...p, bot: true, isBot: true } : p,
  );
}

export function isCustomChampSelectSession(
  session: {
    isCustomGame?: boolean;
    myTeam?: ChampSelectPlayer[];
    theirTeam?: ChampSelectPlayer[];
  },
  rawMyTeam: ChampSelectPlayer[],
  rawTheirTeam: ChampSelectPlayer[],
): boolean {
  return (
    session.isCustomGame === true ||
    rawMyTeam.some((p) => p.isHumanoid) ||
    rawTheirTeam.some((p) => p.isHumanoid)
  );
}

/** 增量合并选人快照：避免过渡帧把已锁定 championId 冲成 0 */
export function mergeChampSelectSnapshot(
  newPlayers: ChampSelectPlayer[],
  prevSnapshot: PremadePlayerLike[],
  session?: ChampSelectSessionLike | null,
): PremadePlayerLike[] {
  return newPlayers.map((p) => {
    const detectedChampId = resolvePlayerChampionId(p, session);
    const prev = prevSnapshot.find(
      (old) =>
        (p.puuid && old.puuid === p.puuid) ||
        (p.summonerId && old.summonerId === p.summonerId) ||
        old.cellId === p.cellId,
    );
    const finalChampId =
      detectedChampId > 0
        ? detectedChampId
        : prev?.championId && prev.championId > 0
          ? prev.championId
          : 0;
    return {
      ...p,
      championId: finalChampId,
    };
  });
}

/** 自定义会话：敌方 cell 可能与我方共用编号，重映射到 5+ 稳定区间 */
export function remapTheirSnapshotForCustom(
  merged: PremadePlayerLike[],
  isCustom: boolean,
): PremadePlayerLike[] {
  if (!isCustom) return merged;
  return merged.map((p, idx) => ({ ...p, cellId: 5 + idx }));
}
