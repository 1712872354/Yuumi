// ── 保留对局数据 localStorage：持续保留上一局数据，直到新对局开始（ChampSelect 时清理）
export const RESERVE_TEAM_KEYS = [
  "yuumi_last_gameflow_my_team",
  "yuumi_last_gameflow_their_team",
  "yuumi_last_game_player_data",
  "yuumi_last_game_loaded_count",
  "yuumi_last_premade_colors_my",
  "yuumi_last_premade_colors_their",
  "yuumi_last_game_id",
  "yuumi_last_game_team_count",
];

export function clearReserveDataFromStorage() {
  try {
    for (const k of RESERVE_TEAM_KEYS) {
      localStorage.removeItem(k);
    }
  } catch {
    /* ignore */
  }
}
