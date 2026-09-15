import { describe, expect, it } from "vitest";
import { mapGameflowParticipant } from "../gameTeamMapping";
import type { GameflowParticipant } from "../../types/lcu";
import type { PlayerData } from "../../types/gameInfo";

function participant(overrides: Partial<GameflowParticipant> = {}): GameflowParticipant {
  return {
    summonerId: 0,
    puuid: "",
    gameName: "",
    tagLine: "",
    summonerName: "",
    displayName: "",
    championId: 0,
    botChampionId: 0,
    bot: false,
    isBot: false,
    profileIconId: undefined,
    cellId: undefined,
    ...overrides,
  } as GameflowParticipant;
}

const emptyCtx = {
  champSelectTeamSnapshot: [],
  champSelectTheirTeamSnapshot: [],
  gameflowMyTeam: [],
  gameflowTheirTeam: [],
  lookupPlayer: (): PlayerData | undefined => undefined,
};

describe("mapGameflowParticipant", () => {
  it("assigns stable cell ids by offset", () => {
    const ally = mapGameflowParticipant(participant({ summonerName: "A" }), 0, 0, false, emptyCtx);
    const enemy = mapGameflowParticipant(participant({ summonerName: "B" }), 0, 5, true, emptyCtx);
    expect(ally.cellId).toBe(0);
    expect(enemy.cellId).toBe(5);
  });

  it("formats name with tag line when present", () => {
    const p = mapGameflowParticipant(
      participant({ gameName: "Riven", tagLine: "KR1" }),
      1,
      0,
      false,
      emptyCtx,
    );
    expect(p.displayName).toBe("Riven#KR1");
  });

  it("inherits champion from previous team data when missing", () => {
    const ctx = {
      ...emptyCtx,
      gameflowMyTeam: [{ cellId: 2, championId: 99, displayName: "x" } as never],
    };
    const p = mapGameflowParticipant(
      participant({ cellId: 2, summonerName: "Me" }),
      2,
      0,
      false,
      ctx,
    );
    expect(p.championId).toBe(99);
  });

  it("inherits puuid/summonerId from champ-select snapshot when session is masked", () => {
    const ctx = {
      ...emptyCtx,
      champSelectTheirTeamSnapshot: [
        {
          cellId: 1,
          puuid: "enemy-puuid",
          summonerId: 424242,
          gameName: "Enemy",
          tagLine: "TW1",
          championId: 103,
          displayName: "Enemy#TW1",
        } as never,
      ],
    };
    // InProgress 脱敏：无 puuid/sid，仅 displayName
    const p = mapGameflowParticipant(
      participant({ summonerName: "Enemy#TW1", championId: 103 }),
      1,
      5,
      true,
      ctx,
    );
    expect(p.puuid).toBe("enemy-puuid");
    expect(p.summonerId).toBe(424242);
    expect(p.gameName).toBe("Enemy");
    expect(p.tagLine).toBe("TW1");
    expect(p.cellId).toBe(6);
  });
});
