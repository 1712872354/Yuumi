import { describe, expect, it } from "vitest";
import type { OpenableLoot } from "../../api/loot";
import {
  getLootDisplayName,
  getFriendlyNameById,
  isKeyFragmentLoot,
  lootPriorityIndex,
} from "../lootNaming";

function loot(overrides: Partial<OpenableLoot> = {}): OpenableLoot {
  return {
    lootId: "CHEST_generic",
    name: "CHEST_generic",
    count: 1,
    needKey: false,
    recipeName: "open",
    tilePath: "",
    ...overrides,
  } as OpenableLoot;
}

describe("lootNaming", () => {
  it("maps known chest ids to Chinese names", () => {
    expect(getLootDisplayName(loot({ lootId: "CHEST_promotion", name: "x" }))).toBe("宝箱");
    expect(getLootDisplayName(loot({ lootId: "CHEST_hextech", name: "x" }))).toBe("海克斯科技宝箱");
    expect(getLootDisplayName(loot({ lootId: "MATERIAL_key_fragment", name: "x" }))).toBe("钥匙碎片");
  });

  it("detects key fragments", () => {
    expect(isKeyFragmentLoot(loot({ lootId: "MATERIAL_key_fragment" }))).toBe(true);
    expect(isKeyFragmentLoot(loot({ lootId: "MATERIAL_key" }))).toBe(false);
  });

  it("priority: promotion < hextech < orb < capsule < other", () => {
    expect(lootPriorityIndex("CHEST_promotion")).toBeLessThan(
      lootPriorityIndex("CHEST_hextech"),
    );
    expect(lootPriorityIndex("ORB_x")).toBeLessThan(lootPriorityIndex("CAPSULE_y"));
    expect(lootPriorityIndex("CAPSULE_y")).toBeLessThan(lootPriorityIndex("UNKNOWN"));
  });

  it("friendly name falls back to lootId", () => {
    expect(getFriendlyNameById([], "FOO")).toBe("FOO");
    expect(
      getFriendlyNameById([{ lootId: "FOO", itemDesc: "描述" } as never], "FOO"),
    ).toBe("描述");
  });
});
