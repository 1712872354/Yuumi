import { describe, expect, it } from "vitest";
import { computePremadeColors } from "../usePremadeGroup";
import type { PremadePlayerLike } from "../../types/gameInfo";

function p(
  id: number,
  opts: Partial<PremadePlayerLike> = {},
): PremadePlayerLike {
  return {
    summonerId: id,
    cellId: id,
    ...opts,
  };
}

describe("computePremadeColors", () => {
  it("returns empty for empty team", () => {
    expect(computePremadeColors([])).toEqual({});
  });

  it("marks solo-party players as -1 and skips those without party id", () => {
    const colors = computePremadeColors([
      p(1), // 无 partyId → 不入表
      p(2, { teamParticipantId: 7 }), // 独自一组 → -1
    ]);
    expect(colors[1]).toBeUndefined();
    expect(colors[2]).toBe(-1);
  });

  it("assigns same color index to premade group members", () => {
    const colors = computePremadeColors([
      p(1, { teamParticipantId: 10 }),
      p(2, { teamParticipantId: 10 }),
      p(3, { teamParticipantId: 20 }),
      p(4, { teamParticipantId: 20 }),
      p(5, { teamParticipantId: 30 }),
    ]);
    expect(colors[1]).toBe(0);
    expect(colors[2]).toBe(0);
    expect(colors[3]).toBe(1);
    expect(colors[4]).toBe(1);
    expect(colors[5]).toBe(-1);
  });

  it("falls back to partyId when teamParticipantId missing", () => {
    const colors = computePremadeColors([
      p(11, { teamParticipantId: undefined, partyId: 99 }),
      p(12, { teamParticipantId: undefined, partyId: 99 }),
    ]);
    expect(colors[11]).toBe(0);
    expect(colors[12]).toBe(0);
  });
});
