import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useI18n } from "vue-i18n";
import { fetchConfig, type AppConfig } from "../api/lcu";
import { useToast } from "./useToast";
import type { Ref } from "vue";

/**
 * OP.GG 独立窗口打开逻辑：已存在则聚焦并可切换英雄，否则按主窗口所在屏幕右侧创建。
 */
export function useOpggWindow(
  appConfig: Ref<AppConfig | null>,
  isConnected: () => boolean,
) {
  const { t } = useI18n();
  const { showToast } = useToast();

  async function openOpggWindow(championId?: number) {
    if (!isConnected()) {
      showToast(t("common.lcuNotConnected"), "warning");
      return;
    }
    // 传递目标英雄给 OP.GG 窗口
    if (championId && championId > 0) {
      try {
        localStorage.setItem("yuumi_opgg_champ", String(championId));
      } catch {
        /* ignore */
      }
    } else {
      try {
        localStorage.removeItem("yuumi_opgg_champ");
      } catch {
        /* ignore */
      }
    }
    const existing = await WebviewWindow.getByLabel("opgg");
    if (existing) {
      await existing.setFocus();
      if (championId && championId > 0) {
        await existing.emit("opgg-select-champion", championId);
      }
      return;
    }

    let alwaysOnTop = false;
    try {
      const cfg = appConfig.value || (await fetchConfig());
      alwaysOnTop = cfg.Functions?.EnableOpggOnTop ?? false;
    } catch (e) {
      console.warn("加载置顶配置失败，使用默认值:", e);
    }

    const savedTheme = localStorage.getItem("yuumi_theme");
    const isSystemDarkNow = window.matchMedia(
      "(prefers-color-scheme: dark)",
    ).matches;
    const nativeTheme: "dark" | "light" =
      savedTheme === "Dark" || (savedTheme !== "Light" && isSystemDarkNow)
        ? "dark"
        : "light";

    const monitor = await currentMonitor();
    if (monitor) {
      const pos = monitor.position.toLogical(monitor.scaleFactor);
      const size = monitor.size.toLogical(monitor.scaleFactor);
      new WebviewWindow("opgg", {
        url: "opgg.html",
        title: "OP.GG",
        width: 760,
        height: 820,
        x: pos.x + size.width - 760 - 2,
        y: pos.y + 2,
        decorations: true,
        resizable: true,
        center: false,
        alwaysOnTop,
        theme: nativeTheme,
      });
    } else {
      const mainPos = await getCurrentWindow().outerPosition();
      const mainSize = await getCurrentWindow().innerSize();
      new WebviewWindow("opgg", {
        url: "opgg.html",
        title: "OP.GG",
        width: 760,
        height: 820,
        x: mainPos.x + mainSize.width + 2,
        y: mainPos.y,
        decorations: true,
        resizable: true,
        center: false,
        alwaysOnTop,
        theme: nativeTheme,
      });
    }
  }

  return { openOpggWindow };
}
