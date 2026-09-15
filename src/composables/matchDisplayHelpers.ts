/** 生涯战绩展示辅助（MatchHistoryTab 共用） */
import { useI18n } from "vue-i18n";
import type { MatchDisplay } from "../api/lcu";
import { isTftQueue } from "../utils/queueMeta";
import { getQueueName as resolveQueueName } from "../utils/queueName";

export function useMatchDisplayHelpers() {
  const { t, te } = useI18n();

  function formatTime(ts: number): string {
    const d = new Date(ts);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  function translateMapName(name: string): string {
    if (!name) return "";
    if (name.includes("峡谷") || name.includes("Rift")) return t("maps.11");
    if (name.includes("深渊") || name.includes("Abyss")) return t("maps.12");
    if (name.includes("闪击") || name.includes("Blitz")) return t("maps.21");
    if (name.includes("大厅") || name.includes("Lobby")) return t("maps.22");
    return name;
  }

  function getQueueName(m: MatchDisplay): string {
    return resolveQueueName(m.queueId, m.name, { t, te });
  }

  function getResultText(m: MatchDisplay): string {
    if (m.placement && isTftQueue(m.queueId)) {
      return `第 ${m.placement} 名`;
    }
    return m.win ? t("career.victory") : t("career.defeat");
  }

  function getResultClass(m: MatchDisplay): string {
    if (m.placement && isTftQueue(m.queueId)) {
      if (m.placement === 1) return "win-text gold-text";
      if (m.placement <= 4) return "win-text";
      return "lose-text";
    }
    return m.win ? "win-text" : "lose-text";
  }

  function getKdaClass(kda: string): string {
    const val = parseFloat(kda);
    if (isNaN(val)) return "kda-perfect";
    if (val >= 5) return "kda-great";
    if (val >= 3) return "kda-good";
    return "kda-normal";
  }

  function getSpellIcon(m: MatchDisplay, slot: 1 | 2): string {
    return slot === 1 ? m.spell1IconUrl : m.spell2IconUrl;
  }

  return {
    formatTime,
    translateMapName,
    getQueueName,
    getResultText,
    getResultClass,
    getKdaClass,
    getSpellIcon,
  };
}
