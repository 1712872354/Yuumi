import type { useGameInfoStore } from "../store/gameInfoStore";
import type { LiveGamePlayer } from "../api/lcu";
import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";
import { isIdentityCompatible } from "./identityUtils";
import { carryChampionFromStaleCell } from "./gameTeamMapping";

export function hasPlayerIdentity(p: PremadePlayerLike): boolean {
  return Boolean(
    p.puuid || p.summonerId || p.displayName || p.gameName || p.summonerName,
  );
}

/**
 * 合并队伍快照：InProgress 下 LCU 可能下发同人数但无身份的“脱敏”列表，
 * 不得用它覆盖选人阶段/保留快照里已识别的玩家；人数更短时保留已有更全数据。
 */
export function mergeTeamPreservingIdentity(
  incoming: PremadePlayerLike[],
  existing: PremadePlayerLike[],
): PremadePlayerLike[] {
  if (incoming.length === 0) return existing;
  if (existing.length === 0) return incoming;
  if (incoming.length < existing.length) return existing;

  const incomingIdentified = incoming.filter(hasPlayerIdentity).length;
  const existingIdentified = existing.filter(hasPlayerIdentity).length;
  if (incomingIdentified === 0 && existingIdentified > 0) {
    console.debug(
      `[GameInfo] InProgress session 无身份信息，保留已有 ${existingIdentified} 名已识别玩家`,
    );
    return existing;
  }

  return incoming.map((p, idx) => {
    if (hasPlayerIdentity(p)) return p;
    const prev =
      existing.find(
        (old) =>
          (p.puuid && old.puuid === p.puuid) ||
          (p.summonerId && old.summonerId === p.summonerId) ||
          (old.cellId !== undefined &&
            old.cellId === p.cellId &&
            hasPlayerIdentity(old)),
      ) || existing[idx];
    if (!prev) return p;
    return {
      ...prev,
      championId: p.championId || prev.championId,
      cellId: p.cellId ?? prev.cellId,
    };
  });
}

export function livePlayerToPremade(
  p: LiveGamePlayer,
  offset: number,
  idx: number,
): PremadePlayerLike {
  const name = p.gameName
    ? p.tagLine
      ? `${p.gameName}#${p.tagLine}`
      : p.gameName
    : p.summonerName || "";
  return {
    summonerId: p.summonerId || undefined,
    puuid: p.puuid || undefined,
    gameName: p.gameName || undefined,
    tagLine: p.tagLine || undefined,
    displayName: name,
    summonerName: p.summonerName || undefined,
    championId: p.championId,
    profileIconId: p.profileIconId || undefined,
    cellId: offset + idx,
    bot: false,
    isBot: false,
  };
}

/** 为双方队员预填充基础占位，避免后台异步请求完成前界面出现空白 */
export function seedInitialPlayerData(
  gameInfo: ReturnType<typeof useGameInfoStore>,
  players: PremadePlayerLike[],
): void {
  for (const p of players) {
    const key = p.cellId ?? 0;
    const isBot = Boolean(p.bot || p.isBot || p.botChampionId);
    const byPuuid = p.puuid ? gameInfo.getPlayer({ puuid: p.puuid }) : undefined;
    const bySid = p.summonerId
      ? gameInfo.getPlayer({ summonerId: p.summonerId })
      : undefined;
    const byCell = gameInfo.getPlayer({ cellId: key });
    const incoming = { puuid: p.puuid, summonerId: p.summonerId, cellId: key };
    const existing = [byPuuid, bySid, byCell].find((e) =>
      isIdentityCompatible(e, incoming),
    );
    if (existing) {
      if (p.championId && p.championId > 0) {
        existing.championId = p.championId;
      }
      gameInfo.setPlayer(existing, {
        cellId: key,
        summonerId: p.summonerId,
        puuid: p.puuid,
      });
      continue;
    }
    if (byCell) {
      if (!p.championId || p.championId <= 0) {
        p.championId = carryChampionFromStaleCell(byCell, key, p.championId);
      }
      gameInfo.deletePlayerByCell(key);
    }
    if (
      !gameInfo.getPlayer({
        cellId: key,
        summonerId: p.summonerId,
        puuid: p.puuid,
      })
    ) {
      const fallbackName =
        p.displayName ||
        p.gameName ||
        p.summonerName ||
        (isBot ? "电脑" : `玩家${key + 1}`);
      const iconId = p.profileIconId ?? 29;
      const placeholder: PlayerData = {
        info: {
          accountId: 0,
          summonerId: p.summonerId ?? 0,
          puuid: p.puuid ?? "",
          displayName: fallbackName,
          gameName: p.gameName || fallbackName,
          tagLine: p.tagLine || "",
          profileIconId: iconId,
          profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
          summonerLevel: 0,
          percentCompleteForNextLevel: 0,
          xpSinceLastLevel: 0,
          xpUntilNextLevel: 0,
        },
        matches: [],
        ranked: { solo: null, flex: null },
        loading: !isBot,
        matchHistoryHidden: isBot,
        championId: p.championId,
      };
      gameInfo.setPlayer(placeholder, {
        cellId: key,
        summonerId: p.summonerId,
        puuid: p.puuid,
      });
    }
  }
}
