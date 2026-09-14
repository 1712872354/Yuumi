import { describe, expect, it } from "vitest";
import {
  shouldWriteReserveData,
  slimPlayerDataForReserve,
} from "../gameReserveStore";
import type { PlayerData } from "../../types/gameInfo";

function makePlayer(puuid: string, matchCount = 0): PlayerData {
  return {
    info: {
      accountId: 0,
      summonerId: 1,
      displayName: puuid,
      gameName: puuid,
      tagLine: "",
      puuid,
      summonerLevel: 30,
      profileIconId: 1,
      profileIconUrl: "",
      percentCompleteForNextLevel: 0,
      xpSinceLastLevel: 0,
      xpUntilNextLevel: 0,
    } as PlayerData["info"],
    matches: Array.from({ length: matchCount }, (_, i) => ({
      gameId: i,
    })) as PlayerData["matches"],
    ranked: { solo: null, flex: null },
    loading: false,
  };
}

describe("shouldWriteReserveData", () => {
  const base = {
    loadedCount: 5,
    gamePhase: "InProgress",
    currentGameId: 100,
    myTeamLength: 5,
    theirTeamLength: 5,
  };

  it("rejects empty team or no loaded players", () => {
    expect(shouldWriteReserveData({ ...base, myTeamLength: 0 })).toBe(false);
    expect(shouldWriteReserveData({ ...base, loadedCount: 0 })).toBe(false);
  });

  it("writes during real game phase", () => {
    expect(shouldWriteReserveData(base)).toBe(true);
    expect(shouldWriteReserveData({ ...base, gamePhase: "GameStart" })).toBe(true);
  });

  it("allows write when data is complete enough outside real game", () => {
    // 无历史 loadedCount 时允许写入
    expect(shouldWriteReserveData({ ...base, gamePhase: "ChampSelect" })).toBe(true);
  });
});

describe("slimPlayerDataForReserve", () => {
  it("drops loading placeholders and caps matches", () => {
    const loaded = makePlayer("a", 50);
    const loading: PlayerData = {
      ...makePlayer("pending"),
      loading: true,
      info: null,
    };
    const slim = slimPlayerDataForReserve({
      a: loaded,
      "pending:1": loading,
    });
    expect(slim.a.matches.length).toBeLessThanOrEqual(5);
    expect(slim.a.info?.puuid).toBe("a");
    expect(slim["pending:1"]).toBeUndefined();
  });
});
