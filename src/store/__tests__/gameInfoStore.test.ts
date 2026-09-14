import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useGameInfoStore } from "../gameInfoStore";
import type { PlayerData } from "../../types/gameInfo";

function emptyPlayer(puuid = ""): PlayerData {
  return {
    info: puuid
      ? ({
          accountId: 0,
          summonerId: 1,
          displayName: "a",
          gameName: "a",
          tagLine: "",
          puuid,
          summonerLevel: 30,
          profileIconId: 1,
          profileIconUrl: "",
          percentCompleteForNextLevel: 0,
          xpSinceLastLevel: 0,
          xpUntilNextLevel: 0,
        } as PlayerData["info"])
      : null,
    matches: [],
    ranked: { solo: null, flex: null },
    loading: false,
  };
}

describe("gameInfoStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("starts empty", () => {
    const s = useGameInfoStore();
    expect(s.loading).toBe(false);
    expect(Object.keys(s.playerData)).toHaveLength(0);
    expect(s.uniquePlayerList()).toHaveLength(0);
  });

  it("setPlayer indexes by puuid and exposes aliases", () => {
    const s = useGameInfoStore();
    const data = emptyPlayer("p1");
    s.setPlayer(data, { cellId: 1, summonerId: 999, puuid: "p1" });
    expect(s.players.p1).toEqual(data);
    expect(s.playerData.p1).toEqual(data);
    expect(s.playerData[1]).toEqual(data);
    expect(s.playerData[999]).toEqual(data);
    expect(s.uniquePlayerList()).toHaveLength(1);
  });

  it("pending slots upgrade to puuid without duplicates", () => {
    const s = useGameInfoStore();
    const placeholder = emptyPlayer("");
    s.setPlayer(placeholder, { cellId: 3 });
    expect(s.players["pending:3"]).toEqual(placeholder);
    expect(s.playerData[3]).toEqual(placeholder);

    const real = emptyPlayer("abc");
    s.setPlayer(real, { cellId: 3, summonerId: 10, puuid: "abc" });
    expect(s.players.abc).toEqual(real);
    expect(s.players["pending:3"]).toBeUndefined();
    expect(s.playerData[3]).toEqual(real);
    expect(s.playerData[10]).toEqual(real);
    expect(s.uniquePlayerList()).toHaveLength(1);
  });

  it("restorePlayers rebuilds from multi-key snapshot", () => {
    const s = useGameInfoStore();
    const a = emptyPlayer("a");
    const b = emptyPlayer("b");
    s.restorePlayers({
      0: a,
      111: a,
      a,
      5: b,
      b,
    });
    expect(s.uniquePlayerList()).toHaveLength(2);
    expect(s.getPlayer({ puuid: "a" })).toEqual(a);
    expect(s.getPlayer({ cellId: 5 })).toEqual(b);
  });

  it("restorePlayers keeps pending: cell binding", () => {
    const s = useGameInfoStore();
    const placeholder = emptyPlayer("");
    s.restorePlayers({
      "pending:3": placeholder,
    });
    expect(s.getPlayer({ cellId: 3 })).toEqual(placeholder);
    expect(s.playerData[3]).toEqual(placeholder);
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
    s.setPlayer(emptyPlayer("x"), { cellId: 0, puuid: "x" });
    s.resetAll();
    expect(s.currentSummonerId).toBe(0);
    expect(s.currentGameId).toBeNull();
    expect(s.isTftMode).toBe(false);
    expect(s.uniquePlayerList()).toHaveLength(0);
  });
});
