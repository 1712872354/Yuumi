<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppConfig } from "../api/lcu";
import { setLocale } from "../i18n";
import OpggModal from "./OpggModal.vue";
import NaiveApiCapture from "./NaiveApiCapture.vue";

function handleClose() {
  getCurrentWindow().close();
}

// 在组件挂载前立即同步主题，避免白屏闪烁
const savedTheme = localStorage.getItem("yuumi_theme");
const isSystemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
const isDark =
  savedTheme === "Dark" || (savedTheme !== "Light" && isSystemDark);
document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");

let unlistenSelect: UnlistenFn | null = null;

onMounted(async () => {
  // 同步语言配置
  try {
    const config = await invoke<AppConfig>("get_config");
    if (config?.Personalization?.Language) {
      setLocale(config.Personalization.Language);
    }
  } catch (e) {
    console.warn("[OpggWindow] 获取本地配置失败:", e);
  }

  // 主窗口「攻略」按钮再次触发时切换英雄
  unlistenSelect = await listen<number>("opgg-select-champion", (event) => {
    const id = Number(event.payload);
    if (id > 0) {
      try {
        localStorage.setItem("yuumi_opgg_champ", String(id));
      } catch {
        /* ignore */
      }
      // 触发 OpggModal 重新读取：通过自定义事件
      window.dispatchEvent(
        new CustomEvent("yuumi-opgg-select-champ", { detail: id }),
      );
    }
  });

  // 禁用右键菜单
  document.addEventListener("contextmenu", (e) => e.preventDefault());

  // 禁用刷新快捷键：F5 / Ctrl+R / Ctrl+Shift+R
  document.addEventListener("keydown", (e) => {
    if (
      e.key === "F5" ||
      (e.ctrlKey && e.key === "r") ||
      (e.ctrlKey && e.shiftKey && e.key === "R")
    ) {
      e.preventDefault();
    }
  });
});

onUnmounted(() => {
  unlistenSelect?.();
});
</script>

<template>
  <n-config-provider>
    <n-message-provider>
      <n-dialog-provider>
        <NaiveApiCapture />
        <OpggModal @close="handleClose" />
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style>
html,
body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  background: var(--bg-color);
  color: var(--text-color);
  transition:
    background 0.25s,
    color 0.25s;
}

::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 4px;
}
::-webkit-scrollbar-thumb:hover {
  background: var(--primary-color);
}
</style>
