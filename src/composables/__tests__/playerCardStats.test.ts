import { describe, expect, it } from "vitest";
import type { MatchDisplay } from "../../api/lcu";
import {
  computeAvgCs,
  computeAvgDamageRatio,
  computeAvgVision,
  formatStreakBadge,
  getKdaClass,
  getWinRateClass,
} from "../playerCardStats";

function match(overrides: Partial<MatchDisplay> = {}): MatchDisplay {
  return {
    queueId: 420,
    gameId: 1,
    time: "",
    shortTime: "",
    name: "",
    map: "",
    duration: "",
    remake: false,
    win: true,
    championId: 1,
    spell1Id: 0,
    spell2Id: 0,
    champLevel: 18,
    kills: 0,
    deaths: 0,
    assists: 0,
    kda: "",
    itemIds: [],
    runeId: 0,
    cs: 0,
    gold: 0,
    timeStamp: 0,
    totalDamage: 0,
    totalDamageTaken: 0,
    totalHeal: 0,
    visionScore: 0,
    championIconUrl: "",
    spell1IconUrl: "",
    spell2IconUrl: "",
    runeIconUrl: "",
    itemIconUrls: [],
    augmentIds: [],
    augmentIconUrls: [],
    augmentNames: [],
    ...overrides,
  };
}

describe("playerCardStats", () => {
  it("computeAvgCs skips remakes and rounds", () => {
    const list = [
      match({ cs: 100, remake: false }),
      match({ cs: 200, remake: false }),
      match({ cs: 999, remake: true }),
    ];
    expect(computeAvgCs(list)).toBe(150);
    expect(computeAvgCs(undefined)).toBeUndefined();
  });

  it("computeAvgVision returns undefined when zero", () => {
    expect(computeAvgVision([match({ visionScore: 0 })])).toBeUndefined();
    expect(computeAvgVision([match({ visionScore: 20 })])).toBe("20.0");
  });

  it("computeAvgDamageRatio is dealt/taken*100", () => {
    expect(
      computeAvgDamageRatio([
        match({ totalDamage: 15000, totalDamageTaken: 10000 }),
      ]),
    ).toBe(150);
    expect(computeAvgDamageRatio([match({ totalDamageTaken: 0 })])).toBeUndefined();
  });

  it("getWinRateClass / getKdaClass thresholds", () => {
    expect(getWinRateClass(55)).toBe("stat-win");
    expect(getWinRateClass(45)).toBe("stat-loss");
    expect(getWinRateClass(50)).toBe("stat-normal");
    expect(getWinRateClass(undefined)).toBe("stat-dim");
    expect(getKdaClass(3)).toBe("stat-win");
    expect(getKdaClass(1.5)).toBe("stat-loss");
    expect(getKdaClass(2.5)).toBe("stat-normal");
  });

  it("formatStreakBadge requires count >= 2", () => {
    expect(formatStreakBadge(null)).toBeNull();
    expect(formatStreakBadge({ type: "win", count: 1 })).toBeNull();
    expect(formatStreakBadge({ type: "win", count: 3 })).toEqual({
      text: "3连胜",
      cls: "badge-win",
    });
    expect(formatStreakBadge({ type: "loss", count: 2 })).toEqual({
      text: "2连败",
      cls: "badge-loss",
    });
  });
});
