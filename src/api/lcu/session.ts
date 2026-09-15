import { invoke } from "@tauri-apps/api/core";
import type { GamePhase, ChampSelectSession } from "../../store/lcuStore";
import { lcuRequest } from "./core";

export interface LiveGamePlayer {
  summonerId: number;
  puuid: string;
  summonerName: string;
  gameName: string;
  tagLine: string;
  championId: number;
  profileIconId: number;
  team: "my" | "their";
}

export interface LiveGameTeams {
  gameId: number | null;
  myTeam: LiveGamePlayer[];
  theirTeam: LiveGamePlayer[];
}

/** 进行中对局双方玩家（Rust 从 gameflow session 解析，前端 session 残缺时兜底） */
export const fetchLiveGameTeams = () =>
  invoke<LiveGameTeams>("get_live_game_teams");

/**
 * 对局中完整双方名单：session summonerId → 并发拉召唤师详情（与 post_game 同源）。
 * InProgress 下 GameInfo 优先用此路径。
 */
export const fetchOngoingGameRoster = () =>
  invoke<LiveGameTeams>("get_ongoing_game_roster");

// ─── LCU API 快捷方法（透传原始 JSON）───

/** 获取游戏阶段 */
export const getGameflowPhase = () =>
  lcuRequest<GamePhase>("GET", "/lol-gameflow/v1/gameflow-phase");

/** 获取选人会话 */
export const getChampSelectSession = () =>
  lcuRequest<ChampSelectSession>("GET", "/lol-champ-select/v1/session");

/** 接受匹配 */
export const acceptMatch = () =>
  lcuRequest<void>("POST", "/lol-matchmaking/v1/ready-check/accept");

/** 修改生涯背景 */
export const setProfileBackground = (skinId: number) =>
  lcuRequest<void>("POST", "/lol-summoner/v1/current-summoner/background-id", {
    key: skinId,
  });

/** 修改头像图标 */
export const setProfileIcon = (iconId: number) =>
  lcuRequest<void>("PUT", "/lol-summoner/v1/current-summoner/icon", {
    profileIconId: iconId,
  });

/** 修改状态签名 */
export const setOnlineStatus = (status: string) =>
  lcuRequest<void>("PUT", "/lol-chat/v1/me", { statusMessage: status });

/** 设置在线状态 (online / away / offline) */
export const setOnlineAvailability = (availability: string) =>
  lcuRequest<void>("PUT", "/lol-chat/v1/me", { availability });

/** 在选人阶段选择英雄 */
export const selectChampion = (actionId: number, championId: number) =>
  lcuRequest<void>(
    "PATCH",
    `/lol-champ-select/v1/session/actions/${actionId}`,
    { championId, completed: true },
  );

/** 在选人阶段禁用英雄 */
export const banChampion = (actionId: number, championId: number) =>
  lcuRequest<void>(
    "PATCH",
    `/lol-champ-select/v1/session/actions/${actionId}`,
    { championId, completed: true },
  );

/** 设置召唤师技能 */
export const setSummonerSpells = (spell1Id: number, spell2Id: number) =>
  lcuRequest<void>("PATCH", "/lol-champ-select/v1/session/my-selection", {
    spell1Id,
    spell2Id,
  });
