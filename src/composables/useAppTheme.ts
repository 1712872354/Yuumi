import { computed, onMounted, ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { darkTheme, type GlobalThemeOverrides } from "naive-ui";
import {
  applyDpiScale,
  toHex6,
  updateCardColors,
  updateDeathColor,
  updateThemeColor,
} from "../utils/theme";
import type { AppConfig } from "../api/lcu";

/** 主题模式写入 localStorage 与 document 根节点 */
export function applyThemeMode(mode: string) {
  const root = document.documentElement;
  if (mode === "Auto") {
    root.removeAttribute("data-theme");
    localStorage.setItem("yuumi_theme", "Auto");
  } else if (mode === "Light" || mode === "Dark") {
    root.setAttribute("data-theme", mode.toLowerCase());
    localStorage.setItem("yuumi_theme", mode);
  }
}

/** 云母效果：同步 DOM 标记并通知 Rust 设置窗口特效 */
export function applyMicaEffect(enabled: boolean) {
  const root = document.documentElement;
  if (enabled) {
    root.setAttribute("data-mica", "true");
  } else {
    root.removeAttribute("data-mica");
  }
  invoke("set_mica_effect", { enabled }).catch((e: unknown) =>
    console.warn("应用云母效果失败:", e),
  );
}

/**
 * 应用外观配置：主题色、胜负卡色、死亡数字颜色、DPI、主题模式、云母。
 * 启动路径使用。
 */
export function applyPersonalizationAppearance(cfg: AppConfig) {
  const p = cfg.Personalization;
  if (!p) return;
  if (p.ThemeColor) updateThemeColor(p.ThemeColor);
  updateCardColors(p.WinCardColor, p.LoseCardColor, p.RemakeCardColor);
  updateDeathColor(p.LightDeathsNumberColor, p.DarkDeathsNumberColor);
  applyDpiScale(p.DpiScale);
  applyThemeMode(p.ThemeMode);
  applyMicaEffect(!!p.MicaEnabled);
}

/** 轻量主题应用（LCU 状态恢复路径：不含死亡色/DPI/云母） */
export function applyThemeColorsOnly(cfg: AppConfig | null) {
  if (!cfg?.Personalization) return;
  const p = cfg.Personalization;
  if (p.ThemeColor) updateThemeColor(p.ThemeColor);
  updateCardColors(p.WinCardColor, p.LoseCardColor, p.RemakeCardColor);
  if (p.ThemeMode) applyThemeMode(p.ThemeMode);
}

/** 系统暗色媒体查询与派生主题判断 + Naive UI 主题覆盖 */
export function useThemeState(appConfig: Ref<AppConfig | null>) {
  const isSystemDark = ref(
    window.matchMedia("(prefers-color-scheme: dark)").matches,
  );

  onMounted(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      isSystemDark.value = e.matches;
    };
    media.addEventListener("change", handler);
  });

  const isDarkTheme = computed(() => {
    const mode = appConfig.value?.Personalization?.ThemeMode || "Auto";
    if (mode === "Dark") return true;
    if (mode === "Light") return false;
    return isSystemDark.value;
  });

  // 水晶极光主题覆盖（动态响应系统主题色）
  const themeOverrides = computed<GlobalThemeOverrides>(() => {
    const customColor = appConfig.value?.Personalization?.ThemeColor
      ? toHex6(appConfig.value.Personalization.ThemeColor)
      : "#a78bfa";

    const isDark = isDarkTheme.value;

    return {
      common: {
        primaryColor: customColor,
        primaryColorHover: customColor + "d9",
        primaryColorPressed: customColor + "a6",
        borderRadius: "10px",
      },
      Card: {
        color: "rgba(255, 255, 255, 0.15)",
        borderColor: "rgba(255, 255, 255, 0.2)",
      },
      Dialog: {
        color: isDark ? "rgba(28, 28, 30, 0.92)" : "rgba(255, 255, 255, 0.90)",
        borderColor: isDark
          ? "rgba(255, 255, 255, 0.12)"
          : "rgba(0, 0, 0, 0.08)",
        iconColorWarning: customColor,
        iconColorError: "#ec4899",
        iconColorSuccess: "#10b981",
        iconColorInfo: customColor,
      },
      Button: {
        textColorPrimary: "#ffffff",
      },
    };
  });

  return { isSystemDark, isDarkTheme, themeOverrides, darkTheme };
}
