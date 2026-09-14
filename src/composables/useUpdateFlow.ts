import { ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UpdateInfo } from "../components/UpdateDialog.vue";
import type { AppConfig } from "../api/lcu";

/**
 * 自动更新状态与事件监听：
 * - pendingUpdateInfo：侧边栏红点 / Settings 卡片
 * - updateInfo：UpdateDialog 可见内容
 */
export function useUpdateFlow(appConfig: Ref<AppConfig | null>) {
  const updateInfo = ref<UpdateInfo | null>(null);
  const pendingUpdateInfo = ref<UpdateInfo | null>(null);
  const hasUpdate = ref(false);
  const updateDialogMinimized = ref(true);
  const isPortable = ref(false);

  /** Settings 手动检查 / 立即更新：推送到 UpdateDialog */
  function showUpdateInfo(info: UpdateInfo, expanded = false) {
    pendingUpdateInfo.value = info;
    updateInfo.value = info;
    hasUpdate.value = true;
    updateDialogMinimized.value = !expanded;
  }

  /** 注册 Rust 推送的更新事件与便携版探测（在 onMounted 中调用） */
  async function setupUpdateListeners() {
    try {
      isPortable.value = await invoke<boolean>("is_portable");
    } catch (e) {
      console.warn("[App] 识别便携版失败:", e);
    }

    // 后台逻辑：startup_check_update 始终检测并 emit，仅当 EnableCheckUpdate=true 时才自动后台下载
    await listen<UpdateInfo>("updater://update-available", (event) => {
      pendingUpdateInfo.value = event.payload;
      hasUpdate.value = true;
      const autoEnabled = appConfig.value?.General?.EnableCheckUpdate ?? false;
      if (autoEnabled) {
        updateDialogMinimized.value = true;
        updateInfo.value = event.payload;
      }
    });

    // 下载完成：若用户关闭过弹窗，重新弹出「更新就绪」气泡
    await listen<UpdateInfo>("updater://download-ready", (event) => {
      pendingUpdateInfo.value = event.payload;
      hasUpdate.value = true;
      if (!updateInfo.value) {
        updateDialogMinimized.value = true;
        updateInfo.value = event.payload;
      }
    });
  }

  return {
    updateInfo,
    pendingUpdateInfo,
    hasUpdate,
    updateDialogMinimized,
    isPortable,
    showUpdateInfo,
    setupUpdateListeners,
  };
}
