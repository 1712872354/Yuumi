import { listen } from "@tauri-apps/api/event";
import type { AppConfig } from "../api/lcu";
import { isBadAutoTag, isGoodAutoTag } from "../api/lcu/savedPlayers";
import { useToast } from "./useToast";
import type { Ref } from "vue";

interface RadarPlayer {
  name: string;
  tag: string;
  autoTag?: string | null;
  manualTag?: string | null;
  relation?: string | null;
  listKind?: string;
  listReason?: string | null;
}

/**
 * 对局雷达：选人阶段发现已知（已标记）队友时 Toast 提示。
 * 在 onMounted 中调用 setupRadarAlertListener。
 */
export function useRadarAlert(appConfig: Ref<AppConfig | null>) {
  const { showToast } = useToast();

  async function setupRadarAlertListener() {
    await listen<{ players: RadarPlayer[] }>("radar-alert", (event) => {
      const players = event.payload?.players || [];
      if (!players.length) return;
      // 黑名单优先且更醒目
      const blacks = players.filter((p) => p.listKind === "black");
      if (blacks.length) {
        for (const p of blacks.slice(0, 2)) {
          const why = p.listReason || p.tag || "已拉黑";
          showToast(`🚫 拉黑玩家 ${p.name}：${why}`, "error");
        }
        const dodgeOn = appConfig.value?.Functions?.EnableDodgeReminder !== false;
        if (dodgeOn) {
          showToast("检测到黑名单队友，可考虑秒退（不会自动秒）", "warning");
        }
      }
      for (const p of players.filter((x) => x.listKind !== "black").slice(0, 3)) {
        const isGood = isGoodAutoTag(p.autoTag) || p.listKind === "white";
        const icon = isGood ? "✓" : isBadAutoTag(p.autoTag) ? "⚠" : "📌";
        const rel =
          p.relation === "enemy" ? "曾对手" : p.relation === "ally" ? "曾同队" : "";
        const parts = [p.tag, rel].filter(Boolean).join(" · ");
        showToast(
          `${icon} 雷达 ${p.name}：${parts || "已标记"}`,
          isGood ? "success" : "warning",
        );
      }
    });
  }

  return { setupRadarAlertListener };
}
