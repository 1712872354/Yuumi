import { beforeEach, describe, expect, it, vi } from "vitest";
import { mergeMatchesWithCache, mergeMatchLists } from "../gameMatchesCache";
import type { MatchDisplay } from "../../api/lcu";

function match(gameId: number, timeStamp: number): MatchDisplay {
  return { gameId, timeStamp } as MatchDisplay;
}

function mockLocalStorage() {
  const store = new Map<string, string>();
  const ls = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => {
      store.set(k, v);
    },
    clear: () => store.clear(),
    removeItem: (k: string) => {
      store.delete(k);
    },
  };
  vi.stubGlobal("localStorage", ls);
  return ls;
}

describe("mergeMatchesWithCache", () => {
  beforeEach(() => {
    mockLocalStorage();
  });

  it("merges dedupes and sorts by timeStamp desc", () => {
    const result = mergeMatchesWithCache("p1", [match(1, 100), match(2, 300)]);
    expect(result.map((m) => m.gameId)).toEqual([2, 1]);

    const again = mergeMatchesWithCache("p1", [match(3, 200)]);
    // 2(ts300) → 3(ts200) → 1(ts100)
    expect(again.map((m) => m.gameId)).toEqual([2, 3, 1]);
  });

  it("caps cached list at 50 entries", () => {
    const fresh = Array.from({ length: 60 }, (_, i) => match(i + 1, 1000 - i));
    const result = mergeMatchesWithCache("p2", fresh);
    expect(result.length).toBe(50);
    expect(result[0]?.gameId).toBe(1); // timeStamp 最大
  });

  it("falls back to career cache when fresh is empty", () => {
    localStorage.setItem(
      "yuumi_matches_cache_p3",
      JSON.stringify([match(9, 500)]),
    );
    const result = mergeMatchesWithCache("p3", []);
    expect(result.map((m) => m.gameId)).toEqual([9]);
  });
});

describe("mergeMatchLists", () => {
  it("dedupes by gameId and keeps fresher entry", () => {
    const merged = mergeMatchLists(
      [match(1, 200)],
      [match(1, 100), match(2, 300)],
      10,
    );
    expect(merged.map((m) => m.gameId)).toEqual([2, 1]);
    expect(merged.find((m) => m.gameId === 1)?.timeStamp).toBe(200);
  });

  it("respects limit", () => {
    const list = Array.from({ length: 5 }, (_, i) => match(i + 1, 100 - i));
    expect(mergeMatchLists(list, [], 2)).toHaveLength(2);
  });
});
