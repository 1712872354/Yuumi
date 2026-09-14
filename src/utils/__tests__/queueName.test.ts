import { describe, expect, it } from "vitest";
import { getQueueName } from "../queueName";

function makeI18n(map: Record<string, string>) {
  return {
    te: (key: string) => key in map,
    t: (key: string) => map[key] ?? key,
  } as unknown as Parameters<typeof getQueueName>[2];
}

describe("getQueueName", () => {
  it("uses translation when available", () => {
    const i18n = makeI18n({ "gameModes.420": "单双排位" });
    expect(getQueueName(420, "Ranked Solo", i18n)).toBe("单双排位");
  });

  it("falls back to backend name when no translation", () => {
    const i18n = makeI18n({});
    expect(getQueueName(9999, "Custom Chaos", i18n)).toBe("Custom Chaos");
  });

  it("falls back when translation is TFT but backend is not", () => {
    const i18n = makeI18n({ "gameModes.1090": "云顶之弈" });
    expect(getQueueName(1090, "Normal Game", i18n)).toBe("Normal Game");
  });

  it("keeps TFT translation when backend is also TFT", () => {
    const i18n = makeI18n({ "gameModes.1100": "云顶之弈（排位）" });
    expect(getQueueName(1100, "TFT Ranked", i18n)).toBe("云顶之弈（排位）");
  });
});
