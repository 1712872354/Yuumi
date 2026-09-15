/** TFT 阵容站位兜底：无 cell 时按前排/后排智能落位 */
import type { TftMetaUnit } from "../composables/useTftMetaDecks";

export function isFrontlineUnit(unit: TftMetaUnit): boolean {
  const key = (unit.characterId ?? unit.key ?? "").toLowerCase();

  const tankKeywords = [
    "blitzcrank", "pantheon", "shen", "tahmkench", "ornn", "riven", "leona",
    "mordekaiser", "aatrox", "ivernminion", "briar", "rhaast", "jax", "talon",
    "nunu", "garen", "poppy", "vi", "sion", "malphite", "galio", "taric", "braum",
    "alistar", "chogath", "darius", "drmundo", "hecarim", "illaoi", "jarvaniv",
    "ksante", "kled", "leesin", "nasus", "nautilus", "olaf", "rammus", "renekton",
    "sejuani", "sett", "shyvana", "singed", "skarner", "swain", "trundle", "udyr",
    "volibear", "warwick", "wukong", "xinzhao", "yorick", "zac",
  ];
  if (tankKeywords.some((k) => key.includes(k))) {
    return true;
  }

  const backlineKeywords = [
    "nami", "jinx", "missfortune", "xayah", "zoe", "viktor", "twistedfate",
    "aurelionsol", "vex", "jhin", "lissandra", "gwen", "ahri", "anivia", "annie",
    "ashe", "azir", "brand", "caitlyn", "cassiopeia", "corki", "draven", "ezreal",
    "heimerdinger", "hwei", "jayce", "kaisa", "karthus", "katarina", "kayle",
    "kogmaw", "leblanc", "lucian", "lux", "malzahar", "morgana", "neeko", "orianna",
    "ryze", "samira", "senna", "seraphine", "smolder", "sona", "soraka", "syndra",
    "taliyah", "teemo", "tristana", "twitch", "varus", "veigar", "velkoz", "xerath",
    "zeri", "ziggs", "zilean",
  ];
  if (backlineKeywords.some((k) => key.includes(k))) {
    return false;
  }

  const items = (unit.items ?? []).filter(Boolean);
  const hasTankItem = items.some((it) => {
    const s = String(it).toLowerCase();
    return (
      s.includes("gargoyle") ||
      s.includes("warmog") ||
      s.includes("bramble") ||
      s.includes("dragon") ||
      s.includes("sunfire") ||
      s.includes("redemption") ||
      s.includes("steadfast") ||
      s.includes("protector") ||
      s.includes("ionic") ||
      s.includes("crownguard")
    );
  });
  if (hasTankItem) return true;

  return unit.isCore || unit.priority === 1;
}

export function computeDisplayBoard(units: TftMetaUnit[] | undefined): {
  units: TftMetaUnit[];
  isFallback: boolean;
} {
  if (!units?.length) {
    return { units: [], isFallback: false };
  }

  const hasValidCells = units.some(
    (u) => u.cell && typeof u.cell.x === "number" && typeof u.cell.y === "number",
  );

  if (hasValidCells) {
    return { units, isFallback: false };
  }

  const result: TftMetaUnit[] = [];
  const occupied = new Set<string>();

  const findFreeCell = (preferredY: number[]): { x: number; y: number } => {
    for (const y of preferredY) {
      for (const x of [4, 3, 5, 2, 6, 1, 7]) {
        const key = `${x},${y}`;
        if (!occupied.has(key)) {
          occupied.add(key);
          return { x, y };
        }
      }
    }
    for (const y of [4, 3, 2, 1]) {
      for (const x of [1, 2, 3, 4, 5, 6, 7]) {
        const key = `${x},${y}`;
        if (!occupied.has(key)) {
          occupied.add(key);
          return { x, y };
        }
      }
    }
    return { x: 1, y: 1 };
  };

  const frontlineUnits: TftMetaUnit[] = [];
  const backlineUnits: TftMetaUnit[] = [];

  units.forEach((u) => {
    if (isFrontlineUnit(u)) {
      frontlineUnits.push(u);
    } else {
      backlineUnits.push(u);
    }
  });

  frontlineUnits.forEach((u) => {
    const copy = { ...u };
    copy.cell = findFreeCell([4, 3, 2]);
    result.push(copy);
  });

  backlineUnits.forEach((u) => {
    const copy = { ...u };
    copy.cell = findFreeCell([1, 2, 3]);
    result.push(copy);
  });

  return { units: result, isFallback: true };
}
