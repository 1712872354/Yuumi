<script setup lang="ts">
import { ref, onMounted, watch, computed, provide } from "vue";
import { useLcuStore, initLcuListeners, type ChampSelectSession } from "./store/lcuStore";
import { storeToRefs } from "pinia";
import { fetchCurrentSummoner, getGameflowPhase, lcuRequest, fetchConfig } from "./api/lcu";
import { useI18n } from "vue-i18n";
import { setLocale } from "./i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SummonerDisplay, AppConfig } from "./api/lcu";
import NaiveApiCapture from "./components/NaiveApiCapture.vue";
import { useToast, getCapturedDialog } from "./composables/useToast";
import {
  applyMicaEffect,
  applyPersonalizationAppearance,
  applyThemeColorsOnly,
  useThemeState,
} from "./composables/useAppTheme";
import { useOpggWindow } from "./composables/useOpggWindow";
import { useUpdateFlow } from "./composables/useUpdateFlow";
import { useRadarAlert } from "./composables/useRadarAlert";
import BenchOverlay from "./views/BenchOverlay.vue";
import NoticePopup from "./components/NoticePopup.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import CustomTitleBar from "./components/layout/CustomTitleBar.vue";
import NavigationSidebar from "./components/layout/NavigationSidebar.vue";
import { buildKeepAlivePages, Home } from "./constants/appPages";

provide("applyMicaEffect", applyMicaEffect);

const store = useLcuStore();
const { gamePhase, currentPage } = storeToRefs(store);
const appConfig = ref<AppConfig | null>(null);
provide("appConfig", appConfig);
const pageHistory: string[] = [];
const isSidebarExpanded = ref(false);
const noticeVisible = ref(false);
const summoner = ref<SummonerDisplay | null>(null);
const platformId = ref("");
const mapSideLabel = ref(""); // 蓝色方/红色方
const { t, te } = useI18n();

// 检测当前是否是悬浮窗窗口（bench-overlay）
const isOverlayWindow = ref(
  window.location.search.includes("window=bench-overlay"),
);

const { isDarkTheme, themeOverrides, darkTheme } = useThemeState(appConfig);

// 自动更新：pendingUpdateInfo → 侧边栏红点；updateInfo → 弹窗内容
const {
  updateInfo,
  pendingUpdateInfo,
  hasUpdate,
  updateDialogMinimized,
  isPortable,
  showUpdateInfo,
  setupUpdateListeners,
} = useUpdateFlow(appConfig);
provide("hasUpdate", hasUpdate);
provide("updateInfo", pendingUpdateInfo);
provide("showUpdateInfo", showUpdateInfo);

// Toast 通知（通过 Naive UI Message API；App.vue 位于 Provider 之上，由捕获实例提供）
const { showToast } = useToast();

const { openOpggWindow } = useOpggWindow(appConfig, () => store.isConnected);
provide("openOpgg", openOpggWindow);

const { setupRadarAlertListener } = useRadarAlert(appConfig);

// 用于 Career → Search 跳转的共享状态
const navigateSearchPayload = ref<{
  name: string;
  gameId: number | null;
} | null>(null);
provide("navigateSearchPayload", navigateSearchPayload);

// 用于跳转到 Career 查看指定召唤师的共享状态
const navigateCareerPayload = ref<{
  puuid: string;
} | null>(null);
provide("navigateCareerPayload", navigateCareerPayload);

// 供子组件跳转页面（store 为 currentPage 唯一真源）
function navigateTo(page: string) {
  store.setCurrentPage(page);
}
provide("navigateTo", navigateTo);

/** 已访问过的保活页（延迟首次挂载） */
const visitedPages = ref<Record<string, boolean>>({});
watch(
  currentPage,
  (val) => {
    if (val) visitedPages.value = { ...visitedPages.value, [val]: true };
  },
  { immediate: true },
);

const keepAlivePages = computed(() =>
  buildKeepAlivePages({
    hideTft: !!appConfig.value?.Functions?.HideTft,
    hideSavedPlayers: !!appConfig.value?.Functions?.HideSavedPlayers,
  }),
);

function shouldMountPage(key: string, hiddenWhen?: boolean) {
  if (hiddenWhen) return false;
  return !!visitedPages.value[key] || currentPage.value === key;
}

const regionName = computed(() => {
  if (!platformId.value) return t("regions.HN1");
  const key = `regions.${platformId.value}`;
  const translated = t(key);
  return translated !== key ? translated : platformId.value;
});

// 监听配置中的语言设置，动态切换 locale
watch(
  () => appConfig.value?.Personalization?.Language,
  (newLang) => {
    if (newLang) {
      setLocale(newLang);
    }
  },
  { immediate: true },
);

onMounted(async () => {
  await initLcuListeners();

  if (isOverlayWindow.value) {
    loadLcuState();
    return;
  }

  // 监听系统托盘菜单导航事件
  await listen<string>("tray-navigate", (event: { payload: string }) => {
    navigate(event.payload);
  });

  await setupUpdateListeners();
  await setupRadarAlertListener();

  // 自动启动 LOL 客户端并按需显示主窗口
  try {
    appConfig.value = await fetchConfig();

    // 检查配置加载时是否有错误（如配置文件损坏已自动恢复）
    const configErr = await invoke<null | string>("get_config_load_error");
    if (configErr) {
      const dialog = getCapturedDialog();
      if (dialog) {
        dialog.error({
          title: "配置文件异常",
          content: configErr,
          positiveText: "确定",
          positiveButtonProps: { type: "primary" },
        });
      } else {
        showToast("配置文件异常:\n" + configErr);
      }
    }
    const cfg = appConfig.value;
    if (cfg?.General?.EnableStartLolWithApp) {
      invoke("launch_lol_client").catch((e: unknown) =>
        console.warn("自动启动 LOL 失败:", e),
      );
    }
    // 如果没有开启“游戏开始最小化”（静默启动），则在组件挂载并完成配置获取后显示窗口
    if (!cfg?.General?.EnableGameStartMinimize) {
      await getCurrentWindow().show();
    }
    if (cfg) {
      applyPersonalizationAppearance(cfg);
    }
  } catch (e) {
    console.warn("[App] 启动配置检查失败:", e);
    // 异常情况下兜底显示窗口，保证软件可用性
    await getCurrentWindow().show();
  }
});

function navigate(page: string) {
  if (page === "notice") {
    noticeVisible.value = true;
    return;
  }
  if (page === "career") {
    navigateCareerPayload.value = { puuid: "" };
  }
  if (currentPage.value !== page) {
    pageHistory.push(currentPage.value);
  }
  store.setCurrentPage(page);
}

function goBack() {
  if (pageHistory.length > 0) {
    store.setCurrentPage(pageHistory.pop()!);
  }
}

function toggleSidebar() {
  isSidebarExpanded.value = !isSidebarExpanded.value;
}

async function loadLcuState() {
  console.log(`[loadLcuState] 开始, isConnected=${store.isConnected}`);
  if (!store.isConnected) return;

  // 等待 1 秒，让 LCU API 完全就绪
  await new Promise((r) => setTimeout(r, 1000));

  // 步骤 1/2/3/5 互不依赖，并行请求以减少启动延迟
  const [summonerResp, _platformResp, phaseResp, cfg] = await Promise.allSettled([
    fetchCurrentSummoner(),
    lcuRequest<string>(
      "GET",
      "/lol-platform-config/v1/namespaces/LoginPlatformLocalization/platformId",
    ),
    getGameflowPhase(),
    appConfig.value ? Promise.resolve(appConfig.value) : fetchConfig(),
  ]);

  // 1. 召唤师
  if (summonerResp.status === "fulfilled") {
    summoner.value = summonerResp.value;
    console.log("[loadLcuState] 召唤师:", summoner.value?.displayName);
  } else {
    console.warn("[loadLcuState] 获取召唤师失败:", summonerResp.reason);
  }

  // 2. 大区平台
  if (_platformResp.status === "fulfilled" && _platformResp.value.success && _platformResp.value.data) {
    platformId.value = _platformResp.value.data;
  }

  // 3. 游戏阶段
  if (phaseResp.status === "fulfilled" && phaseResp.value.success && phaseResp.value.data) {
    store.setGamePhase(phaseResp.value.data);
  }

  // 4. 选人 Session（依赖步骤 3 的结果）
  if (store.gamePhase === "ChampSelect") {
    try {
      const sessionResp = await lcuRequest<ChampSelectSession>(
        "GET",
        "/lol-champ-select/v1/session",
      );
      if (sessionResp.success && sessionResp.data) {
        store.setChampSelectSession(sessionResp.data);
      }
    } catch (e) {
      console.warn("[loadLcuState] 获取选人 Session 失败:", e);
    }
  }

  // 5. 主题色
  try {
    const config = cfg.status === "fulfilled" ? cfg.value : null;
    applyThemeColorsOnly(config);
  } catch (e) {
    console.warn("[loadLcuState] 加载配置失败:", e);
  }

  console.log(
    "[loadLcuState] 完成, gamePhase=",
    store.gamePhase,
    "summoner=",
    summoner.value?.displayName,
  );
}

watch(
  () => store.isConnected,
  (connected) => {
    if (connected) {
      loadLcuState();
      // 客户端连接成功后自动跳转到生涯页面
      currentPage.value = "career";
    } else {
      summoner.value = null;
      platformId.value = "";
      mapSideLabel.value = ""; // 断开连接时清空队伍阵营信息
      // 断开连接时回到首页
      currentPage.value = "home";
    }
  },
  { immediate: true },
);

// 监听 Career → Search 跳转
watch(navigateSearchPayload, (payload) => {
  if (payload && payload.gameId !== null) {
    currentPage.value = "search";
  }
});

// 监听跳转到 Career
watch(navigateCareerPayload, (payload) => {
  if (payload?.puuid) {
    currentPage.value = "career";
  }
});

// currentPage 已为 store 单一真源；visitedPages 在 setup 顶部维护

// 防止 session 高频事件重复请求开启悬浮窗（Rust 侧已按 300ms 节流，这里再做一次性守卫）
let benchOverlayRequested = false;

async function showBenchOverlay(show: boolean = true) {
  if (!show) benchOverlayRequested = false;
  // 检查配置开关
  if (show && appConfig.value?.Functions?.EnableBenchOverlay === false) {
    return;
  }
  try {
    await invoke("show_bench_overlay_window", { show });
  } catch (err) {
    console.error("[bench] 控制悬浮窗失败:", err);
  }
}

// lcu-client-started 事件触发时重新加载（游戏中重启等场景）
// isConnected watcher 已覆盖此场景，无需额外监听

// 游戏阶段变化 → 更新窗口标题 + 自动跳转对局信息页
watch(gamePhase, (phase: string) => {
  if (isOverlayWindow.value) return;
  if (import.meta.env.DEV) {
    console.log("[watch gamePhase] phase changed:", phase);
  }

  // 更新窗口标题栏显示游戏状态
  const label = te("phase." + phase) ? t("phase." + phase) : phase;
  const title = label ? `Yuumi · ${label}` : "Yuumi";
  const setTitle = (t: string) =>
    getCurrentWindow()
      .setTitle(t)
      .catch(() => {});

  if (phase === "ChampSelect") {
    // 异步获取队伍信息（蓝色方/红色方）追加到标题
    (async () => {
      try {
        const side = await invoke<string | null>("get_map_side");
        if (import.meta.env.DEV) {
          console.log("[watch gamePhase] get_map_side result:", side);
        }
        if (side) {
          const sideLabel =
            side === "blue" ? t("titlebar.blueSide") : t("titlebar.redSide");
          mapSideLabel.value = sideLabel;
          setTitle(`Yuumi · ${label} - ${sideLabel}`);
          return;
        }
      } catch (e) {
        console.warn("[watch gamePhase] get_map_side failed:", e);
      }
      setTitle(`Yuumi · ${label}`);
    })();
  } else if (phase === "GameStart" || phase === "InProgress") {
    // 游戏加载或进行中时，如果已经存了红蓝方标识，则标题保持带红蓝方的格式，否则尝试再异步拉取一次（如中途重启）
    if (mapSideLabel.value) {
      setTitle(`Yuumi · ${label} - ${mapSideLabel.value}`);
    } else {
      (async () => {
        try {
          const side = await invoke<string | null>("get_map_side");
          if (side) {
            const sideLabel =
              side === "blue" ? t("titlebar.blueSide") : t("titlebar.redSide");
            mapSideLabel.value = sideLabel;
            setTitle(`Yuumi · ${label} - ${sideLabel}`);
            return;
          }
        } catch {
          /* ignore */
        }
        setTitle(`Yuumi · ${label}`);
      })();
    }
  } else {
    // 离开活跃对局（如 Lobby, None, EndOfGame 等）时清除阵营数据并复原标题
    mapSideLabel.value = "";
    setTitle(title);
  }

  // 进入选人/游戏加载/游戏中时自动跳转到对局信息页
  if (
    phase === "ChampSelect" ||
    phase === "GameStart" ||
    phase === "InProgress"
  ) {
    if (import.meta.env.DEV) {
      console.log("[watch gamePhase] navigating to gameinfo");
    }
    currentPage.value = "gameinfo";

    if (phase === "ChampSelect") {
      const runAutoShow = async () => {
        try {
          const cfg = appConfig.value || (await fetchConfig());
          if (cfg?.Functions?.AutoShowOpgg) {
            openOpggWindow();
          }
        } catch (e) {
          console.warn("读取配置用于自动弹出 OP.GG 失败:", e);
        }
      };
      runAutoShow();
    }
  }

  // ─── 大乱斗板凳席悬浮窗生命周期控制 ───
  if (phase === "ChampSelect") {
    setTimeout(async () => {
      const session = store.champSelectSession;
      if (session && session.benchEnabled) {
        benchOverlayRequested = true;
        await showBenchOverlay();
      }
    }, 1500);
  } else {
    // 离开选人阶段，关闭悬浮窗
    showBenchOverlay(false);
  }
});

// 动态监听选人会话变化，双重保证在大乱斗模式（板凳席开启）时自动拉起悬浮窗
watch(
  () => store.champSelectSession,
  async (session) => {
    if (isOverlayWindow.value) return;
    if (store.gamePhase === "ChampSelect" && session && session.benchEnabled) {
      if (!benchOverlayRequested) {
        benchOverlayRequested = true;
        await showBenchOverlay();
      }
    }
  },
);

async function reconnectWithRetry(maxAttempts = 5) {
  for (let i = 0; i < maxAttempts; i++) {
    const r = await lcuRequest("POST", "/lol-gameflow/v1/reconnect");
    if (r.success) return r;
    if (i < maxAttempts - 1) {
      await new Promise((resolve) => setTimeout(resolve, 500 * Math.pow(2, i)));
    }
  }
  return { success: false, error: `${t("common.reconnectFailed")}（已重试 ${maxAttempts} 次）` };
}

function handleReconnect() {
  initLcuListeners();
  // 先查询当前游戏阶段，按情况处理
  getGameflowPhase()
    .then(async (resp) => {
      if (!resp.success) {
        showToast(t("common.lcuNotConnected"), "warning");
        return;
      }
      const phase = resp.data;
      if (
        phase === "InProgress" ||
        phase === "GameStart" ||
        phase === "Reconnect"
      ) {
        // 游戏中 → 调用 reconnect API（含指数退避重试）
        const r = await reconnectWithRetry();
        if (r.success) {
          showToast("🔄 " + t("common.reconnectTriggered"));
        } else {
          showToast(
            t("common.reconnectFailed") + ": " + (r.error || ""),
            "error",
          );
        }
      } else {
        showToast(
          t("common.lcuReset") +
            " (" +
            (te("phase." + (phase ?? ""))
              ? t("phase." + (phase ?? ""))
              : (phase ?? "")) +
            ")",
        );
      }
    })
    .catch(() => {
      showToast("LCU 监听服务已重置");
    });
}

async function handleClose() {
  try {
    const closeToTray = await invoke<boolean>("get_close_to_tray");
    const win = getCurrentWindow();
    if (closeToTray) {
      await win.hide();
    } else {
      await win.close();
    }
  } catch (e) {
    console.error("[handleClose] 失败，直接关闭窗口:", e);
    await getCurrentWindow().close();
  }
}
</script>

<template>
  <n-config-provider
    :theme-overrides="themeOverrides"
    :theme="isDarkTheme ? darkTheme : null"
  >
    <n-message-provider>
      <n-dialog-provider>
        <NaiveApiCapture />

        <!-- 如果是悬浮窗窗口，仅渲染悬浮窗组件 -->
        <div v-if="isOverlayWindow" class="overlay-container">
          <BenchOverlay />
        </div>

        <!-- 否则渲染常规的主程序界面 -->
        <div v-else class="app-layout">
          <!-- 自定义标题栏 -->
          <CustomTitleBar
            :page-history="pageHistory"
            :game-phase="gamePhase"
            :map-side-label="mapSideLabel"
            @go-back="goBack"
            @close="handleClose"
          />

          <!-- 主体区域：侧边栏 + 内容 -->
          <div class="main-row">
            <NavigationSidebar
              :current-page="currentPage"
              :is-sidebar-expanded="isSidebarExpanded"
              :app-config="appConfig"
              :summoner="summoner"
              :region-name="regionName"
              :has-update="hasUpdate"
              @navigate="navigate"
              @toggle-sidebar="toggleSidebar"
              @open-opgg="() => openOpggWindow()"
              @reconnect="handleReconnect"
            />

            <!-- 右侧内容区域 -->
            <main class="content-wrapper">
            <!-- 保活业务页：v-show + 延迟挂载，元数据见 constants/appPages.ts -->
            <div
              v-for="page in keepAlivePages"
              :key="page.key"
              v-show="currentPage === page.key"
              class="page-slot"
            >
              <component
                :is="page.component"
                v-if="shouldMountPage(page.key, page.hiddenWhen)"
              />
            </div>

              <!-- Home 轻量页面直接 v-if 渲染 -->
              <Home v-if="currentPage === 'home'" @navigate="navigate" />

              <!-- 内建 OP.GG 占位页面 -->
              <div v-if="currentPage === 'opgg'" class="placeholder-view">
                <div class="view-header">
                  <h2>{{ $t("common.opgg") }}</h2>
                </div>
                <div class="view-card">
                  <div class="avatar-circle op-icon">OP</div>
                  <h3>{{ $t("common.opgg") }}</h3>
                  <p>{{ $t("common.opggProxyInfo") }}</p>
                  <div class="status-box">
                    <span class="dot online"></span>
                    <span
                      >{{ $t("common.opggProxyAddress") }}127.0.0.1:7897</span
                    >
                  </div>
                  <p class="hint">{{ $t("common.opggHint") }}</p>
                </div>
              </div>
            </main>
          </div>
        </div>

        <NoticePopup v-if="noticeVisible" @close="noticeVisible = false" />

        <!-- 自动更新弹窗（在 app-layout 外部，避免 overflow:hidden 限制） -->
        <UpdateDialog
          v-if="updateInfo"
          :update-info="updateInfo"
          :portable="isPortable"
          :minimized="updateDialogMinimized"
          @dismiss="updateInfo = null"
          @update:minimized="(v) => (updateDialogMinimized = v)"
        />
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: var(--bg-color-gradient);
}

html[data-mica="true"] .app-layout {
  background: transparent !important;
}

.main-row {
  display: flex;
  flex: 1;
  min-height: 0;
}

.page-slot {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  min-height: 0;
}

/* 右侧内容区域 */
.content-wrapper {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  background-color: transparent;
}

/* 占位页面样式 */
.placeholder-view {
  padding: 3rem 2rem;
  max-width: 840px;
  margin: 0 auto;
}

.view-header {
  margin-bottom: 2rem;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 1.2rem;
}

.view-header h2 {
  font-size: 1.75rem;
  margin: 0;
  font-weight: 800;
  color: var(--text-color);
  letter-spacing: 0.5px;
}

.view-card {
  background: var(--card-bg);
  border-radius: var(--radius-lg);
  padding: 3.5rem 2.5rem;
  text-align: center;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-md);
  transition: all 0.35s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.view-card:hover {
  border-color: var(--primary-color-alpha-40);
  box-shadow:
    0 12px 30px -10px var(--primary-color-alpha-15),
    var(--shadow-lg);
  transform: translateY(-4px);
}

.avatar-circle.op-icon {
  width: 54px;
  height: 54px;
  line-height: 50px;
  border: 2px solid var(--primary-color);
  color: var(--primary-color);
  font-size: 1.3rem;
  font-weight: 900;
  border-radius: 50%;
  margin: 0 auto 1.5rem;
  text-align: center;
  box-shadow: 0 4px 10px var(--primary-color-alpha-15);
}

.view-card h3 {
  font-size: 1.25rem;
  margin: 0 0 0.8rem;
  font-weight: 700;
  color: var(--text-color);
}

.view-card p {
  color: var(--text-muted);
  font-size: 0.88rem;
  line-height: 1.6;
  max-width: 480px;
  margin: 0 auto 1.5rem;
}

.status-box {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: var(--win-bg);
  color: var(--win-color);
  padding: 6px 14px;
  border-radius: 20px;
  font-size: 0.8rem;
  font-weight: 600;
  margin-bottom: 1.5rem;
  border: 1px solid var(--win-border);
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.dot.online {
  background: var(--win-color);
  box-shadow: 0 0 6px var(--win-color);
}

.view-card .hint {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 公告板 */
.changelog-card {
  background: var(--card-bg);
  border-radius: var(--radius-lg);
  padding: 2rem;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
  transition: all 0.3s ease;
}

.changelog-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md);
}

.version-tag {
  display: inline-block;
  background: var(--primary-color);
  color: white;
  padding: 3px 10px;
  border-radius: 4px;
  font-weight: 700;
  font-size: 0.75rem;
  margin-bottom: 0.8rem;
  box-shadow: 0 4px 10px var(--primary-color-alpha-30);
}

.changelog-card h3 {
  margin: 0 0 4px;
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-color);
}

.changelog-card .date {
  color: var(--text-dimmed);
  font-size: 0.78rem;
  margin: 0 0 1.5rem;
}

.changelog-list {
  padding-left: 18px;
  margin: 0;
}

.changelog-list li {
  margin-bottom: 0.8rem;
  color: var(--text-muted);
  line-height: 1.6;
  font-size: 0.85rem;
}

.changelog-list strong {
  color: var(--text-color);
}
</style>
