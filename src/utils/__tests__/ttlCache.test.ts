import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TtlCache } from "../ttlCache";

describe("TtlCache", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("expires entries after ttl", () => {
    const cache = new TtlCache<string>(1000, 10);
    cache.set("a", "1");
    expect(cache.get("a")).toBe("1");
    vi.advanceTimersByTime(1001);
    expect(cache.get("a")).toBeNull();
  });

  it("promotes hot keys on get so they are not evicted first", () => {
    const cache = new TtlCache<number>(10_000, 2);
    cache.set("hot", 1);
    cache.set("cold", 2);
    // 多次访问 hot，使其升到 Map 末尾
    expect(cache.get("hot")).toBe(1);
    expect(cache.get("hot")).toBe(1);
    cache.set("new", 3); // 容量满，应淘汰 cold 而非 hot
    expect(cache.get("hot")).toBe(1);
    expect(cache.get("cold")).toBeNull();
    expect(cache.get("new")).toBe(3);
  });

  it("updates existing key without growing or evicting", () => {
    const cache = new TtlCache<number>(10_000, 2);
    cache.set("a", 1);
    cache.set("b", 2);
    cache.set("a", 10); // 更新已有 key
    expect(cache.get("a")).toBe(10);
    expect(cache.get("b")).toBe(2);
  });
});
