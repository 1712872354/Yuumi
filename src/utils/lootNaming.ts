/** 战利品显示名与优先级（纯函数） */
import type { LootItem, OpenableLoot } from "../api/loot";

export function getLootDisplayName(loot: OpenableLoot): string {
  const name = loot.name;
  const id = loot.lootId;
  if (id === "CHEST_promotion") return "宝箱";
  if (id === "CHEST_champion_mastery" || id === "CHEST_generic" || id === "CHEST_hextech")
    return "海克斯科技宝箱";
  if (id === "CHEST_premium") return "杰作宝箱";
  if (id.toLowerCase() === "chest_128" || name.toLowerCase() === "chest_128")
    return "英雄魔法引擎";
  if (id.toLowerCase() === "chest_129" || name.toLowerCase() === "chest_129")
    return "荣耀英雄魔法引擎";
  if (id === "MATERIAL_key_fragment") return "钥匙碎片";
  if (id === "MATERIAL_key") return "海克斯科技钥匙";
  if (id === "MATERIAL_key_premium") return "杰作钥匙";
  if (name === id) {
    if (id.includes("ORB") || id.includes("orb")) return "法球";
    if (id.includes("CAPSULE")) return "引擎/胶囊";
  }
  return name;
}

export function isKeyFragmentLoot(loot: OpenableLoot): boolean {
  return loot.lootId === "MATERIAL_key_fragment";
}

export function lootPriorityIndex(lootId: string): number {
  if (lootId === "CHEST_promotion") return 0;
  if (
    lootId === "CHEST_champion_mastery" ||
    lootId === "CHEST_generic" ||
    lootId === "CHEST_hextech"
  )
    return 1;
  if (lootId.includes("ORB") || lootId.includes("orb")) return 2;
  if (lootId.includes("CAPSULE")) return 3;
  return 4;
}

export function getFriendlyNameById(
  inventory: LootItem[],
  lootId: string,
): string {
  const found = inventory.find((i) => i.lootId === lootId);
  return found?.itemDesc ?? lootId;
}
