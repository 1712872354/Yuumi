import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { PlayerData, PremadePlayerLike } from "../types/gameInfo";

export interface PlayerIdentityKeys {
  cellId?: number;
  summonerId?: number;
  puuid?: string;
}

function normalizePuuid(puuid?: string | null): string {
  return (puuid || "").trim();
}

function playerKeyFor(
  data: PlayerData,
  keys: PlayerIdentityKeys,
): string {
  const puuid = normalizePuuid(keys.puuid || data.info?.puuid);
  if (puuid) return puuid;
  const cell = keys.cellId;
  return cell !== undefined ? `pending:${cell}` : `pending:unknown`;
}

/**
 * GameInfo 页对局玩家数据运行时状态。
 *
 * 内部以 **puuid 为唯一主键**（无身份时用 `pending:{cellId}`），
 * cellId / summonerId 只作别名索引。对外仍暴露多键 `playerData` 视图，
 * 兼容现有组件按 cellId/sid/puuid 直接下标的读取方式。
 */
export const useGameInfoStore = defineStore("gameInfo", () => {
  const loading = ref(false);
  const error = ref("");
  const currentSummonerId = ref(0);
  const currentSummonerPuuid = ref("");

  /** 主键表：puuid | pending:{cellId} → PlayerData */
  const players = ref<Record<string, PlayerData>>({});
  /** 别名：cell:N / sid:N / puuid:xxx → 主键 */
  const identityIndex = ref<Record<string, string>>({});

  const champSelectTeamSnapshot = ref<PremadePlayerLike[]>([]);
  const champSelectTheirTeamSnapshot = ref<PremadePlayerLike[]>([]);

  const sessionAllyTeam = ref<PremadePlayerLike[]>([]);
  const sessionEnemyTeam = ref<PremadePlayerLike[]>([]);

  const gameflowMyTeam = ref<PremadePlayerLike[]>([]);
  const gameflowTheirTeam = ref<PremadePlayerLike[]>([]);

  const currentQueueId = ref<number | null>(null);
  const isTftMode = ref(false);
  const currentGameId = ref<number | null>(null);

  /** 兼容视图：同一 PlayerData 可通过 cellId / summonerId / puuid 命中 */
  const playerData = computed<Record<string | number, PlayerData>>(() => {
    const out: Record<string | number, PlayerData> = {};
    for (const [key, data] of Object.entries(players.value)) {
      out[key] = data;
      if (key.startsWith("pending:")) {
        const cell = Number(key.slice("pending:".length));
        if (!Number.isNaN(cell)) out[cell] = data;
      }
    }
    for (const [alias, target] of Object.entries(identityIndex.value)) {
      const data = players.value[target];
      if (!data) continue;
      if (alias.startsWith("cell:")) {
        out[Number(alias.slice(5))] = data;
      } else if (alias.startsWith("sid:")) {
        out[Number(alias.slice(4))] = data;
      } else if (alias.startsWith("puuid:")) {
        out[alias.slice(6)] = data;
      }
    }
    return out;
  });

  function setPlayer(data: PlayerData, keys: PlayerIdentityKeys = {}) {
    const key = playerKeyFor(data, keys);
    // 升级：pending 槽获得真实 puuid 时，丢掉旧 pending 主键
    if (!key.startsWith("pending:") && keys.cellId !== undefined) {
      const old = identityIndex.value[`cell:${keys.cellId}`];
      if (old?.startsWith("pending:")) {
        const nextPlayers = { ...players.value };
        delete nextPlayers[old];
        players.value = nextPlayers;
      }
    }
    players.value = { ...players.value, [key]: data };

    const idx = { ...identityIndex.value };
    if (keys.cellId !== undefined) idx[`cell:${keys.cellId}`] = key;
    if (keys.summonerId && keys.summonerId !== keys.cellId) {
      idx[`sid:${keys.summonerId}`] = key;
    }
    const puuid = normalizePuuid(keys.puuid || data.info?.puuid);
    if (puuid) idx[`puuid:${puuid}`] = key;
    identityIndex.value = idx;
  }

  function getPlayer(ref: PlayerIdentityKeys): PlayerData | undefined {
    const puuid = normalizePuuid(ref.puuid);
    if (puuid && players.value[puuid]) return players.value[puuid];
    if (puuid) {
      const key = identityIndex.value[`puuid:${puuid}`];
      if (key && players.value[key]) return players.value[key];
    }
    if (ref.summonerId) {
      const key = identityIndex.value[`sid:${ref.summonerId}`];
      if (key && players.value[key]) return players.value[key];
    }
    if (ref.cellId !== undefined) {
      const key =
        identityIndex.value[`cell:${ref.cellId}`] || `pending:${ref.cellId}`;
      if (players.value[key]) return players.value[key];
    }
    return undefined;
  }

  function deletePlayerByCell(cellId: number) {
    const key =
      identityIndex.value[`cell:${cellId}`] || `pending:${cellId}`;
    const nextPlayers = { ...players.value };
    delete nextPlayers[key];
    delete nextPlayers[`pending:${cellId}`];
    players.value = nextPlayers;

    const idx = { ...identityIndex.value };
    delete idx[`cell:${cellId}`];
    for (const [alias, target] of Object.entries(idx)) {
      if (target === key) delete idx[alias];
    }
    identityIndex.value = idx;
  }

  function uniquePlayerList(): PlayerData[] {
    return Object.values(players.value);
  }

  function clearPlayers() {
    players.value = {};
    identityIndex.value = {};
  }

  /** 从旧版多键快照恢复（localStorage 兼容） */
  function restorePlayers(map: Record<string | number, PlayerData>) {
    clearPlayers();
    const seen = new Set<PlayerData>();
    for (const [rawKey, data] of Object.entries(map)) {
      if (!data || seen.has(data)) continue;
      seen.add(data);
      let cellId: number | undefined;
      if (/^\d+$/.test(rawKey)) {
        cellId = Number(rawKey);
      } else if (rawKey.startsWith("pending:")) {
        const n = Number(rawKey.slice("pending:".length));
        if (!Number.isNaN(n)) cellId = n;
      }
      setPlayer(data, {
        cellId,
        summonerId: data.info?.summonerId,
        puuid: data.info?.puuid,
      });
    }
  }

  function resetPlayerDetail() {
    clearPlayers();
  }

  function resetTeams() {
    champSelectTeamSnapshot.value = [];
    champSelectTheirTeamSnapshot.value = [];
    sessionAllyTeam.value = [];
    sessionEnemyTeam.value = [];
    gameflowMyTeam.value = [];
    gameflowTheirTeam.value = [];
  }

  function resetAll() {
    loading.value = false;
    error.value = "";
    currentSummonerId.value = 0;
    currentSummonerPuuid.value = "";
    resetPlayerDetail();
    resetTeams();
    currentQueueId.value = null;
    isTftMode.value = false;
    currentGameId.value = null;
  }

  return {
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    players,
    identityIndex,
    playerData,
    champSelectTeamSnapshot,
    champSelectTheirTeamSnapshot,
    sessionAllyTeam,
    sessionEnemyTeam,
    gameflowMyTeam,
    gameflowTheirTeam,
    currentQueueId,
    isTftMode,
    currentGameId,
    setPlayer,
    getPlayer,
    deletePlayerByCell,
    uniquePlayerList,
    clearPlayers,
    restorePlayers,
    resetPlayerDetail,
    resetTeams,
    resetAll,
  };
});
