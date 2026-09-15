import { describe, expect, it } from "vitest";
import {
  setPlayerDataAliases,
  uniquePlayerEntries,
  isValidPuuid,
  EMPTY_PUUID,
} from "../identityUtils";
import type { PlayerData } from "../../types/gameInfo";

describe("isValidPuuid", () => {
  it("rejects empty and LCU placeholder uuid", () => {
    expect(isValidPuuid(undefined)).toBe(false);
    expect(isValidPuuid(null)).toBe(false);
    expect(isValidPuuid("")).toBe(false);
    expect(isValidPuuid("  ")).toBe(false);
    expect(isValidPuuid(EMPTY_PUUID)).toBe(false);
  });

  it("accepts real puuid", () => {
    expect(isValidPuuid("abc-123")).toBe(true);
    expect(isValidPuuid(` ${EMPTY_PUUID}x`)).toBe(true);
  });
});

function emptyData(): PlayerData {
  return {
    info: null,
    matches: [],
    ranked: { solo: null, flex: null },
    loading: false,
  };
}

describe("setPlayerDataAliases / uniquePlayerEntries", () => {
  it("writes cellId summonerId and puuid as aliases of one object", () => {
    const map: Record<string | number, PlayerData> = {};
    const data = emptyData();
    setPlayerDataAliases(map, data, {
      cellId: 3,
      summonerId: 999,
      puuid: "abc",
    });
    expect(map[3]).toBe(data);
    expect(map[999]).toBe(data);
    expect(map.abc).toBe(data);
    expect(uniquePlayerEntries(map)).toHaveLength(1);
  });

  it("skips summonerId when it equals cellId", () => {
    const map: Record<string | number, PlayerData> = {};
    const data = emptyData();
    setPlayerDataAliases(map, data, { cellId: 5, summonerId: 5, puuid: "x" });
    expect(map[5]).toBe(data);
    expect(uniquePlayerEntries(map)).toHaveLength(1);
  });

  it("counts multiple distinct players once each", () => {
    const map: Record<string | number, PlayerData> = {};
    const a = emptyData();
    const b = emptyData();
    setPlayerDataAliases(map, a, { cellId: 0, summonerId: 1, puuid: "a" });
    setPlayerDataAliases(map, b, { cellId: 1, summonerId: 2, puuid: "b" });
    expect(uniquePlayerEntries(map)).toHaveLength(2);
  });
});
