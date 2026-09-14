import { describe, expect, it } from "vitest";
import { shouldWriteReserveData } from "../gameReserveStore";

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
