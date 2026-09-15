import { describe, expect, it } from "vitest";
import {
  computeMatchStats,
  computeStatsSummary,
  computeStreak,
  isLikelyBotPlayer,
} from "../gamePlayerStats";
import type { MatchDisplay } from "../../api/lcu";

function match(overrides: Partial<MatchDisplay> = {}): MatchDisplay {
  return {
    gameId: 1,
    queueId: 420,
    kills: 0,
    deaths: 0,
    assists: 0,
    win: false,
    remake: false,
    timeStamp: 0,
    ...overrides,
  } as MatchDisplay;
}

describe("computeMatchStats", () => {
  it("returns undefined stats for empty list", () => {
    const s = computeMatchStats([]);
    expect(s.avgKda).toBeUndefined();
    expect(s.winRate).toBeUndefined();
  });

  it("computes kda win rate and excludes remakes", () => {
    const stats = computeMatchStats([
      match({ kills: 5, deaths: 5, assists: 5, win: true }),
      match({ kills: 0, deaths: 5, assists: 0, win: false }),
      match({ remake: true, win: true }),
    ]);
    expect(stats.winCount).toBe(1);
    expect(stats.lossesCount).toBe(1);
    expect(stats.winRate).toBe(50);
    expect(stats.avgKda).toBeCloseTo((5 + 0 + 5 + 0) / 10);
  });

  it("avoids divide by zero deaths", () => {
    const stats = computeMatchStats([match({ kills: 3, deaths: 0, assists: 2, win: true })]);
    expect(stats.avgKda).toBeCloseTo(5);
  });
});

describe("computeStreak", () => {
  it("returns null for short streaks", () => {
    expect(computeStreak([match({ win: true })])).toBeNull();
    expect(
      computeStreak([match({ win: true }), match({ win: false })]),
    ).toBeNull();
  });

  it("returns win streak of 2+", () => {
    expect(
      computeStreak([match({ win: true }), match({ win: true }), match({ win: false })]),
    ).toEqual({ type: "win", count: 2 });
  });

  it("returns loss streak of 2+", () => {
    expect(
      computeStreak([match({ win: false }), match({ win: false })]),
    ).toEqual({ type: "loss", count: 2 });
  });
});

describe("isLikelyBotPlayer", () => {
  it("detects explicit bot flags", () => {
    expect(
      isLikelyBotPlayer({
        fallbackBot: true,
        realSummonerId: 123,
        playerPuuid: "x",
      }),
    ).toBe(true);
    expect(
      isLikelyBotPlayer({
        isHumanoid: true,
        realSummonerId: 123,
        playerPuuid: "x",
      }),
    ).toBe(true);
  });

  it("detects 电脑 name without identity", () => {
    expect(
      isLikelyBotPlayer({
        displayName: "电脑1",
        realSummonerId: 0,
        playerPuuid: "",
      }),
    ).toBe(true);
  });

  it("does not flag name-only players as bots", () => {
    expect(
      isLikelyBotPlayer({
        displayName: "RivenMain",
        realSummonerId: 0,
        playerPuuid: "",
      }),
    ).toBe(false);
  });

  it("does not flag normal players", () => {
    expect(
      isLikelyBotPlayer({
        displayName: "RivenMain",
        realSummonerId: 999,
        playerPuuid: "abc",
      }),
    ).toBe(false);
  });
});

describe("computeStatsSummary", () => {
  it("returns null for empty list", () => {
    expect(computeStatsSummary([])).toBeNull();
  });

  it("aggregates wins kda and top champions", () => {
    const summary = computeStatsSummary([
      match({
        kills: 10,
        deaths: 2,
        assists: 8,
        win: true,
        championId: 432,
        championIconUrl: "/a.png",
      }),
      match({
        kills: 0,
        deaths: 5,
        assists: 1,
        win: false,
        championId: 432,
        championIconUrl: "/a.png",
      }),
    ]);
    expect(summary?.wins).toBe(1);
    expect(summary?.losses).toBe(1);
    expect(summary?.kills).toBe(10);
    expect(summary?.deaths).toBe(7);
    expect(summary?.kda).toBe("2.7");
    expect(summary?.topChamps[0]?.id).toBe(432);
    expect(summary?.topChamps[0]?.count).toBe(2);
  });
});
