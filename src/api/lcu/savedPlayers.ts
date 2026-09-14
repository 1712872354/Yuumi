import { invoke } from "@tauri-apps/api/core";

// ─── 保存的玩家 ───

export interface SavedPlayer {
  puuid: string;
  selfPuuid: string;
  region: string;
  rsoPlatformId: string;
  tag: string | null;
  summonerName: string;
  profileIconId: number;
  /** Riot ID 的 tagLine（如 NA1），自动相遇记录时采集 */
  tagLine: string | null;
  /** 上次相遇对局使用的英雄 id，0 表示未知 */
  championId: number;
  updateAt: number;
  lastMetAt: number | null;
  lastQueueType: string | null;
  encounterCount: number;
  /** 最近自动标签（稳定 key：carry/carryDamage/feeder/inter/carried；兼容旧中文串） */
  autoTag?: string | null;
  autoScore?: number | null;
  autoReason?: string | null;
  autoTagStats?: string | null;
  lastRelation?: string | null;
  listKind?: string;
  listReason?: string | null;
}

export interface EncounteredGame {
  id: number;
  gameId: number;
  puuid: string;
  selfPuuid: string;
  region: string;
  rsoPlatformId: string;
  queueType: string;
  updateAt: number;
}

export interface PageResult<T> {
  data: T[];
  count: number;
}

export interface SavedPlayerMarker {
  tag: string | null;
  encounterCount: number;
  autoTag?: string | null;
  /** ally / enemy */
  lastRelation?: string | null;
  lastMetAt?: number | null;
  /** '' | black | white */
  listKind?: string;
  listReason?: string | null;
}

// ─── 自动标签 key / 展示名（DB 与事件契约只存 key，兼容旧中文串）───

const AUTO_TAG_KEY_TO_LABEL: Record<string, string> = {
  carry: "大腿",
  carryDamage: "C位",
  feeder: "坑",
  inter: "演员",
  carried: "躺赢",
};

const AUTO_TAG_LABEL_TO_KEY: Record<string, string> = Object.fromEntries(
  Object.entries(AUTO_TAG_KEY_TO_LABEL).map(([k, v]) => [v, k]),
);

/** 归一化自动标签：旧中文串 → 稳定 key；已是 key 则原样返回 */
export function normalizeAutoTagKey(tag?: string | null): string | null {
  if (!tag) return null;
  return AUTO_TAG_LABEL_TO_KEY[tag] ?? tag;
}

/** key → 展示文案；未知值原样返回 */
export function autoTagLabel(tag?: string | null): string {
  if (!tag) return "";
  return AUTO_TAG_KEY_TO_LABEL[tag] ?? tag;
}

export function isGoodAutoTag(tag?: string | null): boolean {
  const key = normalizeAutoTagKey(tag);
  return key === "carry" || key === "carryDamage";
}

export function isBadAutoTag(tag?: string | null): boolean {
  const key = normalizeAutoTagKey(tag);
  return key === "feeder" || key === "inter";
}

export interface SaveSavedPlayerInput {
  puuid: string;
  selfPuuid: string;
  rsoPlatformId?: string;
  region?: string;
  tag?: string | null;
  summonerName?: string;
  profileIconId?: number;
  encountered?: boolean;
  championId?: number;
  /** Riot ID tagLine；空/缺省时后端保留已有值 */
  tagLine?: string;
}

/** 分页查询保存的玩家，filter: "tagged" | "multiple" | undefined */
export const queryAllSavedPlayers = (
  selfPuuid: string,
  page?: number,
  pageSize?: number,
  filter?: string
) =>
  invoke<PageResult<SavedPlayer>>("query_all_saved_players", {
    selfPuuid,
    page,
    pageSize,
    filter,
  });

/** 获取全部保存玩家的精简映射：puuid → 标记信息（对局信息页徽章用） */
export const querySavedPlayersMap = (selfPuuid: string) =>
  invoke<Record<string, SavedPlayerMarker>>("get_saved_players_map", {
    selfPuuid,
  });

/** 分页查询相遇记录 */
export const queryEncounteredGames = (
  selfPuuid: string,
  puuid: string,
  queueType?: string,
  page?: number,
  pageSize?: number
) =>
  invoke<PageResult<EncounteredGame>>("query_encountered_games", {
    selfPuuid,
    puuid,
    queueType,
    page,
    pageSize,
  });

/** 保存玩家 / 更新 tag */
export const saveSavedPlayer = (dto: SaveSavedPlayerInput) =>
  invoke<void>("save_saved_player", { dto });

/** 设置黑白名单：kind = '' | 'black' | 'white' */
export const setPlayerListKind = (
  selfPuuid: string,
  puuid: string,
  kind: string,
  reason?: string | null,
  summonerName?: string | null,
) =>
  invoke<void>("set_player_list_kind", {
    selfPuuid,
    puuid,
    kind,
    reason: reason ?? null,
    summonerName: summonerName ?? null,
  });

/** 回填保存玩家的召唤师 ID（tagLine），返回更新的数量 */
export const backfillSavedPlayerIdentity = () =>
  invoke<number>("backfill_saved_player_identity");

/** 删除保存的玩家 */
export const deleteSavedPlayer = (puuid: string, selfPuuid: string) =>
  invoke<void>("delete_saved_player", { puuid, selfPuuid });

/** 导出带标记玩家为 JSON 文件，取消时返回 null */
export const exportTaggedPlayersToJsonFile = () =>
  invoke<string | null>("export_tagged_players_to_json_file");

/** 从 JSON 文件导入带标记玩家，返回导入数量 */
export const importTaggedPlayersFromJsonFile = () =>
  invoke<number>("import_tagged_players_from_json_file");
