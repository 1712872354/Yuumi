import { invoke } from "@tauri-apps/api/core";

export interface CherryAugmentDetail {
  id: number;
  name: string;
  iconPath: string;
  description: string;
}

export interface MatchDisplay {
  queueId: number;
  gameId: number;
  time: string;
  shortTime: string;
  name: string;
  map: string;
  duration: string;
  remake: boolean;
  win: boolean;
  placement?: number;
  championId: number;
  spell1Id: number;
  spell2Id: number;
  champLevel: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: string;
  itemIds: number[];
  runeId: number;
  cs: number;
  gold: number;
  timeStamp: number;
  totalDamage: number;
  totalDamageTaken: number;
  totalHeal: number;
  visionScore: number;
  championIconUrl: string;
  spell1IconUrl: string;
  spell2IconUrl: string;
  runeIconUrl: string;
  itemIconUrls: string[];
  augmentIds: number[];
  augmentIconUrls: string[];
  augmentNames: string[];
}

/** 获取战绩列表（Rust 解析层清洗后） */
export const fetchMatchHistory = (
  puuid: string,
  begIndex?: number,
  endIndex?: number,
) => invoke<MatchDisplay[]>("get_match_history", { puuid, begIndex, endIndex });

/** 通过 SGP 接口获取战绩列表（支持分页，仅腾讯国服可用） */
export const fetchMatchHistorySgp = (
  puuid: string,
  begIndex: number,
  endIndex: number,
) =>
  invoke<MatchDisplay[]>("get_match_history_sgp", {
    puuid,
    begIndex,
    endIndex,
  });

/**
 * 智能获取战绩列表：委托 Rust `get_match_history_merged` 内部完成 LCU+SGP 合并。
 * 首屏自动并发 SGP（腾讯国服实时源）；`forceSgp` 可强制非首屏也拉 SGP。
 */
export async function fetchMatchHistorySmart(
  puuid: string,
  begIndex = 0,
  endIndex = 19,
  options?: {
    forceSgp?: boolean;
  },
): Promise<MatchDisplay[]> {
  try {
    return await invoke<MatchDisplay[]>("get_match_history_merged", {
      puuid,
      begIndex,
      endIndex,
      forceSgp: options?.forceSgp ?? false,
    });
  } catch (e) {
    console.debug(`[fetchMatchHistorySmart] 合并战绩拉取失败 (puuid: ${puuid}):`, e);
    return [];
  }
}

export interface RecentTeammate {
  name: string;
  puuid: string;
  icon: string;
  total: number;
  wins: number;
  losses: number;
  lastPlayTime: number;
  tag?: string | null;
}

export interface RecentTeammatesResponse {
  puuid: string;
  summoners: RecentTeammate[];
}

export interface PlayerFateInfo {
  fateFlag: "ally" | "enemy" | null;
  recentlyChampionName: string | null;
}

/** 获取最近队友统计 */
export const fetchRecentTeammates = (gameIds: number[], puuid: string) =>
  invoke<RecentTeammatesResponse>("get_recent_teammates", { gameIds, puuid });

/** 获取单个玩家上一局与自己的宿命关系及英雄名 */
export const fetchPlayerFateInfo = (
  gameId: number,
  targetPuuid: string,
  currentSummonerId: number,
) =>
  invoke<PlayerFateInfo>("get_player_fate_info", {
    gameId,
    targetPuuid,
    currentSummonerId,
  });
