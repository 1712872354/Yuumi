import { describe, expect, it } from "vitest";
import type { ChampSelectPlayer } from "../../store/lcuStore";
import {
  buildTeamSig,
  flagBots,
  isCustomChampSelectSession,
  mergeChampSelectSnapshot,
  remapTheirSnapshotForCustom,
} from "../champSelectSnapshot";

function player(p: Partial<ChampSelectPlayer>): ChampSelectPlayer {
  return {
    cellId: 0,
    championId: 0,
    championPickIntent: 0,
    assignedPosition: "",
    ...p,
  } as ChampSelectPlayer;
}

describe("champSelectSnapshot", () => {
  it("buildTeamSig includes cell champ and puuid", () => {
    const sig = buildTeamSig([
      player({ cellId: 1, championId: 432, puuid: "a" }),
    ]);
    expect(sig).toBe("1:432:a");
  });

  it("flagBots marks isHumanoid without changing ownership", () => {
    const out = flagBots([player({ cellId: 0, isHumanoid: true })]);
    expect(out[0]?.bot).toBe(true);
    expect(out[0]?.isBot).toBe(true);
  });

  it("isCustomChampSelectSession detects custom flag or humanoid", () => {
    expect(
      isCustomChampSelectSession({ isCustomGame: true }, [], []),
    ).toBe(true);
    expect(
      isCustomChampSelectSession({}, [player({ isHumanoid: true })], []),
    ).toBe(true);
    expect(isCustomChampSelectSession({}, [player({})], [])).toBe(false);
  });

  it("mergeChampSelectSnapshot keeps previous champion when new is 0", () => {
    const merged = mergeChampSelectSnapshot(
      [player({ cellId: 2, championId: 0, puuid: "x" })],
      [{ cellId: 2, puuid: "x", championId: 99 }],
    );
    expect(merged[0]?.championId).toBe(99);
  });

  it("remapTheirSnapshotForCustom offsets cellId only for custom", () => {
    const base = [{ cellId: 0 }, { cellId: 1 }] as never;
    expect(remapTheirSnapshotForCustom(base, false)).toBe(base);
    const remapped = remapTheirSnapshotForCustom(base, true);
    expect(remapped[0]?.cellId).toBe(5);
    expect(remapped[1]?.cellId).toBe(6);
  });
});
