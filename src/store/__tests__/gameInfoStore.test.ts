import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useGameInfoStore } from "../gameInfoStore";

describe("gameInfoStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("starts empty and can write player data", () => {
    const s = useGameInfoStore();
    expect(s.loading).toBe(false);
    expect(Object.keys(s.playerData)).toHaveLength(0);
    s.playerData[1] = {
      info: {
        summonerId: 1,
        displayName: "a",
        gameName: "a",
        tagLine: null,
        puuid: "p1",
        summonerLevel: 30,
        profileIconId: 1,
        profileIconUrl: "",
        championId: 0,
        isSelf: true,
        isBot: false,
        matchHistory: [],
        mastery: null,
        ranked: null,
        fate: null,
      },
      loading: false,
    } as never;
    expect(Object.keys(s.playerData)).toHaveLength(1);
  });

  it("resetTeams clears team arrays but keeps summoner identity", () => {
    const s = useGameInfoStore();
    s.currentSummonerPuuid = "p1";
    s.gameflowMyTeam = [{ cellId: 0 } as never];
    s.sessionEnemyTeam = [{ cellId: 5 } as never];
    s.resetTeams();
    expect(s.gameflowMyTeam).toHaveLength(0);
    expect(s.sessionEnemyTeam).toHaveLength(0);
    expect(s.currentSummonerPuuid).toBe("p1");
  });

  it("resetAll clears identity and game context", () => {
    const s = useGameInfoStore();
    s.currentSummonerId = 9;
    s.currentGameId = 123;
    s.isTftMode = true;
    s.resetAll();
    expect(s.currentSummonerId).toBe(0);
    expect(s.currentGameId).toBeNull();
    expect(s.isTftMode).toBe(false);
  });
});
