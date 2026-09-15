import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";
import { lazySetItem } from "../utils/lazyStorage";

export const RESERVE_KEYS = {
  playerData: "yuumi_last_game_player_data",
  myTeam: "yuumi_last_gameflow_my_team",
  theirTeam: "yuumi_last_gameflow_their_team",
  premadeMy: "yuumi_last_premade_colors_my",
  premadeTheir: "yuumi_last_premade_colors_their",
  loadedCount: "yuumi_last_game_loaded_count",
  gameId: "yuumi_last_game_id",
  teamCount: "yuumi_last_game_team_count",
} as const;

export interface ReserveWriteInput {
  loadedCount: number;
  gamePhase: string;
  currentGameId: number | null;
  myTeamLength: number;
  theirTeamLength: number;
}

function readNumber(key: string): number {
  try {
    return Number(localStorage.getItem(key)) || 0;
  } catch {
    return 0;
  }
}

/**
 * 判断是否应写入 reserve 快照：
 * - 无队伍或无已加载玩家 → 不写
 * - 非真实对局阶段且数据规模缩水 → 不写（防选人半加载覆盖上一局）
 * - 同一局内更瘦快照 → 不写（防 InProgress 残缺 session 毒化 GameStart 完整快照）
 */
export function shouldWriteReserveData(input: ReserveWriteInput): boolean {
  if (input.myTeamLength === 0 || input.loadedCount === 0) {
    return false;
  }

  const savedLoadedCount = readNumber(RESERVE_KEYS.loadedCount);
  const inRealGame = input.gamePhase === "GameStart" || input.gamePhase === "InProgress";
  if (!inRealGame && input.loadedCount < savedLoadedCount) {
    return false;
  }

  const currentTotal = input.myTeamLength + input.theirTeamLength;
  if (input.currentGameId) {
    const savedGameId = readNumber(RESERVE_KEYS.gameId);
    const savedTotal = readNumber(RESERVE_KEYS.teamCount);
    if (savedGameId === input.currentGameId && savedTotal > 0 && currentTotal < savedTotal) {
      return false;
    }
  }
  return true;
}

/**
 * 当前 live gameId 与保留盘中记录的上一局是否不同。
 * 用于启动恢复 / InProgress 加载时识别「新对局」，避免把上一局 roster 合并进当前局。
 */
export function isDifferentReserveGame(
  liveGameId: number | null | undefined,
  savedGameId: number,
): boolean {
  if (liveGameId == null || liveGameId === 0) return false;
  if (savedGameId === 0) return false;
  return savedGameId !== liveGameId;
}

export function readSavedReserveGameId(): number {
  return readNumber(RESERVE_KEYS.gameId);
}

export interface ReserveSnapshot {
  playerData: Record<string | number, PlayerData>;
  myTeam: PremadePlayerLike[];
  theirTeam: PremadePlayerLike[];
  premadeColorsMy: Record<number, number>;
  premadeColorsTheir: Record<number, number>;
  loadedCount: number;
  gameId: number | null;
}

/**
 * 保留盘落盘瘦身：限制 matches 体积与加载中占位，避免 10 人局撑爆 localStorage。
 * 保留少量最近对局，恢复后仍能展示战绩摘要。
 */
export function slimPlayerDataForReserve(
  map: Record<string | number, PlayerData>,
): Record<string, PlayerData> {
  const MAX_RESERVE_MATCHES = 5;
  const MAX_RESERVE_MASTERIES = 20;
  const out: Record<string, PlayerData> = {};
  for (const [key, data] of Object.entries(map)) {
    if (!data?.info || data.loading) continue;
    out[key] = {
      info: data.info,
      matches: (data.matches || []).slice(0, MAX_RESERVE_MATCHES),
      ranked: data.ranked,
      loading: false,
      matchHistoryHidden: data.matchHistoryHidden,
      championId: data.championId,
      avgKda: data.avgKda,
      winRate: data.winRate,
      winCount: data.winCount,
      lossesCount: data.lossesCount,
      fateFlag: data.fateFlag,
      recentlyChampionName: data.recentlyChampionName,
      masteries: data.masteries?.slice(0, MAX_RESERVE_MASTERIES),
      streak: data.streak,
    };
  }
  return out;
}

export function writeReserveSnapshotToStorage(snapshot: ReserveSnapshot) {
  const slimPlayers = slimPlayerDataForReserve(snapshot.playerData);
  lazySetItem(RESERVE_KEYS.playerData, slimPlayers);
  lazySetItem(RESERVE_KEYS.myTeam, snapshot.myTeam);
  lazySetItem(RESERVE_KEYS.theirTeam, snapshot.theirTeam);
  lazySetItem(RESERVE_KEYS.premadeMy, snapshot.premadeColorsMy);
  lazySetItem(RESERVE_KEYS.premadeTheir, snapshot.premadeColorsTheir);
  lazySetItem(RESERVE_KEYS.loadedCount, snapshot.loadedCount);
  if (snapshot.gameId) {
    lazySetItem(RESERVE_KEYS.gameId, snapshot.gameId);
    lazySetItem(RESERVE_KEYS.teamCount, snapshot.myTeam.length + snapshot.theirTeam.length);
  }
}

export interface LoadedReserveSnapshot {
  myTeam: PremadePlayerLike[] | null;
  theirTeam: PremadePlayerLike[] | null;
  playerData: Record<string | number, PlayerData> | null;
  premadeColorsMy: Record<number, number> | null;
  premadeColorsTheir: Record<number, number> | null;
}

export function readReserveSnapshotFromStorage(): LoadedReserveSnapshot {
  const empty: LoadedReserveSnapshot = {
    myTeam: null,
    theirTeam: null,
    playerData: null,
    premadeColorsMy: null,
    premadeColorsTheir: null,
  };
  try {
    const savedMyTeam = localStorage.getItem(RESERVE_KEYS.myTeam);
    const savedTheirTeam = localStorage.getItem(RESERVE_KEYS.theirTeam);
    const savedPlayerData = localStorage.getItem(RESERVE_KEYS.playerData);
    const savedPremadeMy = localStorage.getItem(RESERVE_KEYS.premadeMy);
    const savedPremadeTheir = localStorage.getItem(RESERVE_KEYS.premadeTheir);

    const result: LoadedReserveSnapshot = { ...empty };
    if (savedMyTeam) {
      const parsed = JSON.parse(savedMyTeam);
      if (Array.isArray(parsed) && parsed.length > 0) result.myTeam = parsed;
    }
    if (savedTheirTeam) {
      const parsed = JSON.parse(savedTheirTeam);
      if (Array.isArray(parsed) && parsed.length > 0) result.theirTeam = parsed;
    }
    if (savedPlayerData) {
      const parsed = JSON.parse(savedPlayerData);
      if (parsed && Object.keys(parsed).length > 0) result.playerData = parsed;
    }
    if (savedPremadeMy) {
      try {
        result.premadeColorsMy = JSON.parse(savedPremadeMy);
      } catch {
        /* keep null, caller recomputes */
      }
    }
    if (savedPremadeTheir) {
      try {
        result.premadeColorsTheir = JSON.parse(savedPremadeTheir);
      } catch {
        /* keep null, caller recomputes */
      }
    }
    return result;
  } catch {
    return empty;
  }
}
