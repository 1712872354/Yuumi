import { describe, it, expect } from "vitest";
import {
  formatRankDisplay,
  formatTierCn,
  QUEUE_FILTER_OPTIONS,
  QUEUE_NAME_MAP,
  TIER_MAP,
} from "../queueMeta";

describe("formatTierCn", () => {
  it("maps known tiers with division", () => {
    expect(formatTierCn("GOLD", "II")).toBe("荣耀黄金II");
  });

  it("omits NA / empty division for master+", () => {
    expect(formatTierCn("MASTER", "NA")).toBe("超凡大师");
    expect(formatTierCn("CHALLENGER")).toBe("最强王者");
  });

  it("returns empty for NONE or missing", () => {
    expect(formatTierCn("NONE")).toBe("");
    expect(formatTierCn(null)).toBe("");
  });
});

describe("formatRankDisplay", () => {
  it("formats tier with space-separated division", () => {
    expect(formatRankDisplay("GOLD", "II")).toBe("荣耀黄金 II");
  });

  it("returns empty placeholder for NONE", () => {
    expect(formatRankDisplay("NONE", "I")).toBe("--");
    expect(formatRankDisplay(null, null)).toBe("--");
  });
});

describe("queue meta", () => {
  it("has null as the all-filter option", () => {
    expect(QUEUE_FILTER_OPTIONS[0]?.id).toBeNull();
  });

  it("includes core ranked tiers", () => {
    expect(TIER_MAP.IRON).toBeTruthy();
    expect(TIER_MAP.CHALLENGER).toBeTruthy();
  });

  it("covers bot and arena queues aligned with Rust", () => {
    expect(QUEUE_NAME_MAP[800]).toBe("人机对战");
    expect(QUEUE_NAME_MAP[850]).toBe("人机对战");
    expect(QUEUE_NAME_MAP[1700]).toBe("斗魂竞技场");
    expect(QUEUE_NAME_MAP[1710]).toBe("斗魂竞技场");
  });
});
