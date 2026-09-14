import type { MatchDetail, MatchDetailTeam, GameDataAssets } from "../types/lcu";
import type { GameDetail, GameDetailPlayer } from "../types/search";
import { QUEUE_NAME_MAP, MAP_NAME_MAP } from "./queueMeta";

interface AugmentSource {
  augments?: number[];
  playerAugment1?: number;
  playerAugment2?: number;
  playerAugment3?: number;
  playerAugment4?: number;
  playerAugment5?: number;
}

function normalizeAssetPath(path: string): string {
  return path.startsWith("/") ? path : "/" + path;
}

function extractAugmentIds(stats: AugmentSource, participant?: AugmentSource): number[] {
  const seen = new Set<number>();
  const ids: number[] = [];
  for (const source of [stats, participant].filter(Boolean) as AugmentSource[]) {
    if (Array.isArray(source.augments)) {
      for (const id of source.augments) {
        if (id && !seen.has(id)) {
          seen.add(id);
          ids.push(id);
        }
      }
    }
    for (let i = 1; i <= 5; i++) {
      const id = source[`playerAugment${i}` as keyof AugmentSource] as number | undefined;
      if (id && !seen.has(id)) {
        seen.add(id);
        ids.push(id);
      }
    }
  }
  return ids.slice(0, 5);
}

/** 将 LCU MatchDetail + 静态资源映射组装为 Search 详情视图模型 */
export function buildGameDetail(
  g: MatchDetail,
  gameDataAssets: GameDataAssets | null,
  queriedPuuid: string | null,
): GameDetail {
  const getSpellUrl = (spellId?: number) => {
    if (!spellId) return "";
    const path = gameDataAssets?.spells?.[spellId];
    if (!path) return "";
    return normalizeAssetPath(path);
  };

  const getRuneUrl = (runeId?: number) => {
    if (!runeId) return "";
    const path = gameDataAssets?.runes?.[runeId];
    if (!path) return "";
    return normalizeAssetPath(path);
  };

  const getItemUrl = (itemId?: number) => {
    if (!itemId) return "";
    const mapped = gameDataAssets?.items?.[itemId];
    if (mapped) return normalizeAssetPath(mapped);
    return `/lol-game-data/assets/v1/items/icons2d/${itemId}.png`;
  };

  const getAugmentUrl = (augmentId: number) => {
    if (!augmentId) return "";
    const detail = gameDataAssets?.augments?.[augmentId];
    if (detail?.iconPath) return normalizeAssetPath(detail.iconPath);
    return "";
  };

  const playerMap: Record<number, { name: string; puuid: string; summonerId: number }> = {};
  if (g.participantIdentities) {
    for (const identity of g.participantIdentities) {
      const pId = identity.participantId;
      const player = identity.player;
      const baseName = player?.gameName || player?.summonerName || "未知";
      const tag = player?.tagLine;
      playerMap[pId] = {
        name: tag ? `${baseName}#${tag}` : baseName,
        puuid: player?.puuid || "",
        summonerId: player?.summonerId ?? 0,
      };
    }
  }

  const bluePlayers: GameDetailPlayer[] = [];
  const redPlayers: GameDetailPlayer[] = [];

  if (g.participants) {
    for (const p of g.participants) {
      const pId = p.participantId;
      const nameInfo = playerMap[pId] || { name: "未知", puuid: "", summonerId: 0 };
      const stats = p.stats || {};

      const itemUrls = [
        getItemUrl(stats.item0),
        getItemUrl(stats.item1),
        getItemUrl(stats.item2),
        getItemUrl(stats.item3),
        getItemUrl(stats.item4),
        getItemUrl(stats.item5),
        getItemUrl(stats.item6),
      ];

      const augmentIds = extractAugmentIds(stats, p);
      const augmentIconUrls: string[] = [];
      const augmentNames: string[] = [];

      for (const id of augmentIds) {
        const url = getAugmentUrl(id);
        if (url) {
          augmentIconUrls.push(url);
          const detail = gameDataAssets?.augments?.[id];
          const name = detail?.name?.trim() ? detail.name : "海克斯强化";
          augmentNames.push(name);
        }
      }

      const pData: GameDetailPlayer = {
        participantId: pId,
        teamId: p.teamId,
        championId: p.championId,
        championIconUrl: `/lol-game-data/assets/v1/champion-icons/${p.championId}.png`,
        spell1Url: getSpellUrl(p.spell1Id),
        spell2Url: getSpellUrl(p.spell2Id),
        runeUrl: getRuneUrl(stats.perk0),
        name: nameInfo.name,
        puuid: nameInfo.puuid,
        summonerId: nameInfo.summonerId,
        level: stats.champLevel,
        kills: stats.kills ?? 0,
        deaths: stats.deaths ?? 0,
        assists: stats.assists ?? 0,
        cs: (stats.totalMinionsKilled ?? 0) + (stats.neutralMinionsKilled ?? 0),
        gold: stats.goldEarned ?? 0,
        damage: stats.totalDamageDealtToChampions ?? 0,
        items: itemUrls.slice(0, 6),
        ward: itemUrls[6],
        win: stats.win,
        augmentIconUrls,
        augmentNames,
      };

      if (p.teamId === 100) {
        bluePlayers.push(pData);
      } else {
        redPlayers.push(pData);
      }
    }
  }

  const isBlueWin = bluePlayers[0]?.win ?? false;
  const blueKills = bluePlayers.reduce((sum, p) => sum + p.kills, 0);
  const redKills = redPlayers.reduce((sum, p) => sum + p.kills, 0);

  const teamsData: MatchDetailTeam[] = g.teams || [];
  const blueTeamRaw = teamsData.find((t) => t.teamId === 100) || ({} as MatchDetailTeam);
  const redTeamRaw = teamsData.find((t) => t.teamId === 200) || ({} as MatchDetailTeam);

  const mins = Math.floor(g.gameDuration / 60);
  const secs = g.gameDuration % 60;
  const durationStr = `${mins}:${secs < 10 ? "0" + secs : secs}`;

  const date = new Date(g.gameCreation);
  const dateStr = `${date.getFullYear()}/${(date.getMonth() + 1).toString().padStart(2, "0")}/${date.getDate().toString().padStart(2, "0")} ${date.getHours().toString().padStart(2, "0")}:${date.getMinutes().toString().padStart(2, "0")}`;

  let isQueriedPlayerWin = false;
  let queriedPlayerChampionIconUrl = "";
  if (queriedPuuid) {
    const allPlayers = [...bluePlayers, ...redPlayers];
    const found = allPlayers.find((p) => p.puuid === queriedPuuid);
    if (found) {
      isQueriedPlayerWin = found.win ?? false;
      queriedPlayerChampionIconUrl = found.championIconUrl;
    }
  }

  const resultStr = isQueriedPlayerWin ? "victory" : "defeat";
  let mapKey = "other";
  if (g.mapId === 11) {
    mapKey = "sr";
  } else if (g.mapId === 12) {
    mapKey = "ha";
  } else if (g.mapId === 30 || g.queueId === 1700) {
    mapKey = "arena";
  }
  const mapIconUrl = `/images/${mapKey}-${resultStr}.png`;

  return {
    gameId: g.gameId,
    queueId: g.queueId,
    mapId: g.mapId,
    duration: durationStr,
    date: dateStr,
    queueName: QUEUE_NAME_MAP[g.queueId] || "自定义模式",
    mapName: MAP_NAME_MAP[g.mapId] || "未知地图",
    win: isQueriedPlayerWin,
    queriedPlayerChampionIconUrl,
    mapIconUrl,
    blue: {
      teamId: 100,
      players: bluePlayers,
      kills: blueKills,
      win: isBlueWin,
      towerKills: blueTeamRaw.towerKills ?? 0,
      inhibitorKills: blueTeamRaw.inhibitorKills ?? 0,
      baronKills: blueTeamRaw.baronKills ?? 0,
      dragonKills: blueTeamRaw.dragonKills ?? 0,
      riftHeraldKills: blueTeamRaw.riftHeraldKills ?? 0,
    },
    red: {
      teamId: 200,
      players: redPlayers,
      kills: redKills,
      win: !isBlueWin,
      towerKills: redTeamRaw.towerKills ?? 0,
      inhibitorKills: redTeamRaw.inhibitorKills ?? 0,
      baronKills: redTeamRaw.baronKills ?? 0,
      dragonKills: redTeamRaw.dragonKills ?? 0,
      riftHeraldKills: redTeamRaw.riftHeraldKills ?? 0,
    },
  };
}
