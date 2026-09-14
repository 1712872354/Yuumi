<script setup lang="ts">
import {
  ref,
  computed,
  onMounted,
  onUnmounted,
  inject,
  watch,
  type Ref,
} from "vue";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../store/lcuStore";
import {
  fetchMatchHistory,
  fetchMatchHistorySgp,
  fetchMatchHistorySmart,
  fetchCurrentSummoner,
  lcuRequest,
  batchUploadMatches,
  fetchConfig,
  updateConfig,
} from "../api/lcu";
import type { SummonerDisplay, MatchDisplay, AppConfig } from "../api/lcu";
import type { GameDataAssets, RawSummoner } from "../types/lcu";
import LcuOfflineState from "../components/LcuOfflineState.vue";
import MiniMatchList from "../components/search/MiniMatchList.vue";
import MatchDetailPanel from "../components/search/MatchDetailPanel.vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useToast } from "../composables/useToast";
import type { GameDetail } from "../types/search";
import { QUEUE_FILTER_OPTIONS } from "../utils/queueMeta";
import { buildGameDetail } from "../utils/gameDetailBuilder";
import { useSearchHistory } from "../composables/useSearchHistory";
import { useMatchDetailCache } from "../composables/useMatchDetailCache";

const store = useLcuStore();
const { t } = useI18n();
const navigateTo = inject<(page: string) => void>("navigateTo")!;
const searchName = ref("");

const { showToast } = useToast();
const searching = ref(false);
const error = ref("");
const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);

// 游戏模式筛选（-1 = 全部；n-select 不接受 null）
const selectedQueue = ref<number>(-1);
const QUEUE_OPTIONS = QUEUE_FILTER_OPTIONS;

// 上传相关
const uploadEnabled = ref(true);

async function onUploadToggle(val: boolean) {
  uploadEnabled.value = val;
  try {
    const cfg = await fetchConfig();
    cfg.Functions.UploadEnabled = uploadEnabled.value;
    await updateConfig(cfg);
  } catch (e) {
    console.error("保存上传配置失败:", e);
  }
}
const uploadedGameIds = ref(new Set<number>());

// 游戏模式筛选后的全量列表
const allFilteredMatches = computed(() => {
  if (selectedQueue.value === -1) return allMatchesSearch.value;
  return allMatchesSearch.value.filter(
    (m: MatchDisplay) => m.queueId === selectedQueue.value,
  );
});

const currentRiotId = computed(() => {
  if (!summoner.value) return "";
  const gn = summoner.value.gameName || summoner.value.displayName;
  const tl = summoner.value.tagLine;
  return tl ? `${gn}#${tl}` : gn;
});

function selectQueue(id: number) {
  selectedQueue.value = id;
  currentPageNum.value = 1;
  loadMatchHistoryList();
  if (matches.value.length > 0) {
    selectMatch(matches.value[0].gameId);
  }
}

// 点击对局中的其他召唤师名称 → 在当前页面搜索（用 summonerId 避免 400/404/422 错误）
const pendingSummonerId = ref<number>(0);

function searchPlayerBySummonerId(summonerId: number, displayName: string) {
  if (!summonerId) return;
  pendingSummonerId.value = summonerId;
  searchName.value = displayName || String(summonerId);
  resetSelection();
  doSearch();
}

const appConfig = ref<AppConfig | null>(null);
const showTierInGameInfo = computed(
  () => appConfig.value?.Functions?.ShowTierInGameInfo ?? false,
);

const gameDataAssets = ref<GameDataAssets | null>(null);

const {
  selectedGameId,
  selectedGame,
  gameLoading,
  participantRanks,
  selectMatch: selectMatchCached,
  resetSelection,
} = useMatchDetailCache();

function selectMatch(gameId: number) {
  return selectMatchCached(gameId, showTierInGameInfo.value);
}

// 分页相关
const currentPageNum = ref(1);
const matchesPerPage = 10;
const hasMore = ref(false); // 是否有下一页
const allMatchesSearch = ref<MatchDisplay[]>([]); // 全量数据，本地翻页用
const loadedGameIndex = ref(0); // 已加载到的游标
const loadingMore = ref(false); // 防止重复触发 SGP 加载
const pageSwitching = ref(false); // 防止翻页期间重复触发
const INITIAL_BATCH = 20; // 首次加载 20 条（2 页）
const LOAD_MORE_COUNT = 30; // 每次增量加载 30 条
const PREFETCH_PAGES = 1; // 提前 1 页预拉取

// 搜索代数：每次发起新搜索自增，用于丢弃旧搜索的后台预取结果，防止串号污染
let searchGeneration = 0;

const {
  showHistory,
  filteredHistory,
  loadSearchHistory,
  saveToHistory,
  removeFromHistory,
  hideHistoryDelayed,
} = useSearchHistory(searchName, currentRiotId);

function selectHistory(name: string) {
  searchName.value = name;
  showHistory.value = false;
  doSearch();
}

// 从 App.vue 注入 Career → Search 跳转状态
const navigateSearchPayload = inject<
  Ref<{ name: string; gameId: number | null } | null>
>("navigateSearchPayload")!;

let unlistenGameDataReady: (() => void) | null = null;

onMounted(async () => {
  loadSearchHistory();
  try {
    const cfg = await fetchConfig();
    appConfig.value = cfg;
    uploadEnabled.value = cfg.Functions?.UploadEnabled ?? true;
  } catch (e) {
    console.warn("加载上传配置失败，使用默认值:", e);
  }
  try {
    unlistenGameDataReady = await listen("game-data-ready", async () => {
      try {
        gameDataAssets.value = await invoke<GameDataAssets>("get_game_data_assets");
      } catch (e) {
        console.error("收到就绪事件后加载静态资源映射失败:", e);
      }
    });
  } catch (e) {
    console.error("订阅 game-data-ready 事件失败:", e);
  }
});

onUnmounted(() => {
  if (unlistenGameDataReady) {
    unlistenGameDataReady();
  }
});

// 监听 LCU 连接状态，当连接成功后重新拉取静态资源映射
watch(
  () => store.isConnected,
  async (connected) => {
    if (connected) {
      try {
        gameDataAssets.value = await invoke<GameDataAssets>("get_game_data_assets");
      } catch (e) {
        console.error("加载静态资源数据映射失败:", e);
      }
    } else {
      gameDataAssets.value = null;
    }
  },
  { immediate: true },
);

// 监听对局结束：若当前搜索的是自己（当前登录玩家），自动重搜刷新最新战绩
watch(
  () => store.gameEndedTrigger,
  async (trigger) => {
    if (!trigger || !summoner.value?.puuid) return;
    try {
      const currentSummoner = await fetchCurrentSummoner();
      if (currentSummoner?.puuid && summoner.value.puuid === currentSummoner.puuid) {
        console.log("[Search] 对局结束，检测到当前查看本人战绩，自动刷新战绩列表");
        await doSearch();
      }
    } catch {
      // 忽略检查异常
    }
  },
);

// 监听 Career → Search 跳转：自动填入名称并搜索，然后选中指定对局
watch(
  navigateSearchPayload,
  async (payload) => {
    if (!payload || !payload.name || payload.gameId === null) return;
    searchName.value = payload.name;
    let started = await doSearch();
    // doSearch 因防重入被拦截（上一搜索仍在进行）：循环等待其结束再补执行本次跳转搜索，
    // 等待超时仍进行中时保留 payload 继续等，避免新跳转被丢弃
    while (!started) {
      if (navigateSearchPayload.value !== payload) return; // 已被更新的跳转覆盖
      const idle = await waitForSearchIdle();
      if (!idle) continue; // 上一搜索超时仍未结束：保留 payload，下一轮继续等待
      // 等待期间可能出现了更新的跳转目标：放弃本次补执行，交由最新跳转的回调处理，避免重复搜索
      if (navigateSearchPayload.value !== payload) return;
      searchName.value = payload.name; // 补执行前重写跳转目标，防止等待期间被手动输入覆盖
      started = await doSearch();
    }
    // doSearch 完成后自动选中 Career 传来的对局（-1 表示只搜索不选中）
    if (payload.gameId > 0 && matches.value.length > 0) {
      selectMatch(payload.gameId);
    }
    // 清除跳转状态，避免后续重复触发
    navigateSearchPayload.value = null;
  },
  { immediate: true },
);

/** 等待当前进行中的搜索结束（轮询 searching，超时返回 false） */
async function waitForSearchIdle(timeoutMs = 30000): Promise<boolean> {
  const start = Date.now();
  while (searching.value && Date.now() - start < timeoutMs) {
    await new Promise((r) => setTimeout(r, 50));
  }
  return !searching.value;
}

async function doSearch(): Promise<boolean> {
  if (!searchName.value.trim()) return false;
  if (searching.value) return false; // 防重入：进行中直接忽略重复点击
  searching.value = true;
  error.value = "";
  searchGeneration++; // 使上一次搜索的后台预取结果失效
  // 保留旧数据作为模糊背景，等拉取成功后再覆盖，防止界面生硬闪烁

  try {
    const name = searchName.value.trim();
    let resp;
    const summonerId = pendingSummonerId.value;
    pendingSummonerId.value = 0;

    if (summonerId) {
      // 通过数字 summonerId 直接查询（从对局详情点击其他玩家时使用）
      resp = await lcuRequest<RawSummoner>(
        "GET",
        `/lol-summoner/v1/summoners/${summonerId}`,
      );
    } else if (name.includes("#")) {
      const hashIndex = name.indexOf("#");
      const gameName = name.slice(0, hashIndex);
      const tagLine = name.slice(hashIndex + 1);
      resp = await lcuRequest<RawSummoner>(
        "GET",
        `/lol-summoner/v1/alias/lookup?gameName=${encodeURIComponent(gameName)}&tagLine=${encodeURIComponent(tagLine)}`,
      );
    } else {
      resp = await lcuRequest<RawSummoner>(
        "GET",
        `/lol-summoner/v1/summoners?name=${encodeURIComponent(name)}`,
      );
    }

    if (!resp.success || !resp.data) {
      error.value = resp.error || "未找到该召唤师";
      summoner.value = null;
      matches.value = [];
      allMatchesSearch.value = [];
      resetSelection();
      return true;
    }

    // 成功后，开始准备赋新值前，清空旧的分页/详情等局部变量
    allMatchesSearch.value = [];
    loadedGameIndex.value = 0;
    resetSelection();
    currentPageNum.value = 1;
    uploadedGameIds.value = new Set();

    const data = resp.data;
    summoner.value = {
      accountId: data.accountId ?? 0,
      displayName: data.displayName ?? name,
      gameName: data.gameName ?? "",
      tagLine: data.tagLine ?? "",
      percentCompleteForNextLevel: data.percentCompleteForNextLevel ?? 0,
      profileIconId: data.profileIconId ?? 29,
      puuid: data.puuid ?? "",
      summonerId: data.summonerId ?? 0,
      summonerLevel: data.summonerLevel ?? 0,
      xpSinceLastLevel: data.xpSinceLastLevel ?? 0,
      xpUntilNextLevel: data.xpUntilNextLevel ?? 0,
      profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${data.profileIconId ?? 29}.jpg`,
    };

    // 搜索成功后保存到历史记录（用 gameName#tagLine 格式，方便下次直接查询）
    const gn = summoner.value.gameName || summoner.value.displayName || name;
    const tl = summoner.value.tagLine;
    const historyKey = tl ? `${gn}#${tl}` : gn;
    saveToHistory(historyKey);

    if (summoner.value.puuid) {
      await loadMatchHistoryList();
      // 双重保障：如果在查询新人数据后，确认第一局战绩正确载入，强制调用 selectMatch 载入右侧详情
      if (matches.value.length > 0) {
        await selectMatch(matches.value[0].gameId);
      }
    }
  } catch (e: unknown) {
    error.value = String(e);
    summoner.value = null;
    matches.value = [];
    allMatchesSearch.value = [];
    resetSelection();
  } finally {
    searching.value = false;
  }
  return true;
}

// 后台预取战绩：使用 setTimeout 让当前帧先渲染，避免阻塞当前交互
let prefetchTimer: ReturnType<typeof setTimeout> | null = null;
function schedulePrefetchMatches() {
  if (prefetchTimer) clearTimeout(prefetchTimer);
  prefetchTimer = setTimeout(() => {
    prefetchTimer = null;
    void loadMoreMatches();
  }, 0);
}

async function loadMatchHistoryList() {
  if (!summoner.value) return;
  try {
    const beg = (currentPageNum.value - 1) * matchesPerPage;
    const end = beg + matchesPerPage;

    // 首次加载一波 + 翻到尽头时增量拉取
    if (allMatchesSearch.value.length === 0) {
      // 首次加载: 并发智能合并 LCU + SGP 战绩（国服下如果 LCU 本地接口刚打完未同步，可秒级拉取 SGP 最新一把）
      const raw = await fetchMatchHistorySmart(
        summoner.value.puuid,
        0,
        INITIAL_BATCH - 1,
      );
      console.log(`[Search] 首次加载: raw.length=${raw.length}`);
      allMatchesSearch.value = raw;
      loadedGameIndex.value = Math.max(INITIAL_BATCH, raw.length);
    } else if (
      currentPageNum.value >= 2 &&
      end + matchesPerPage * PREFETCH_PAGES >= allFilteredMatches.value.length
    ) {
      // 用户翻到第 2 页或更后页时，若剩余数据不足则提前后台预取后续数据
      schedulePrefetchMatches();
    }

    // 从筛选后的全量数据中切片当前页（保证每页都是精确的 10 条）
    matches.value = allFilteredMatches.value.slice(beg, end);
    // 只要当前筛选总数大于 end，就允许向后翻页
    hasMore.value = allFilteredMatches.value.length > end;

    console.log(
      `[Search] 第${currentPageNum.value}页: gameId=${matches.value[0]?.gameId}, 共${matches.value.length}条, allFiltered=${allFilteredMatches.value.length}, hasMore=${hasMore.value}`,
    );

    // 自动批量上传当前页对局（去重 + fire-and-forget）
    if (uploadEnabled.value && matches.value.length > 0) {
      const newIds = matches.value
        .map((m: MatchDisplay) => m.gameId)
        .filter((id: number) => !uploadedGameIds.value.has(id));
      if (newIds.length > 0) {
        newIds.forEach((id: number) => uploadedGameIds.value.add(id));
        console.log(`[upload] 开始批量上传 ${newIds.length} 场对局:`, newIds);
        batchUploadMatches(newIds)
          .then(
            (result: {
              successCount: number;
              failedCount: number;
              error: string | null;
            }) => {
              console.log(
                `[upload] 批量上传结果: 成功=${result.successCount}, 失败=${result.failedCount}, error=${result.error}`,
              );
              if (result.error && result.successCount === 0) {
                showToast(`上传失败: ${result.error}`, "error");
              } else if (result.successCount > 0) {
                showToast(`已上传 ${result.successCount} 场对局`);
              } else {
                showToast("所有对局已存在，无需上传");
              }
            },
          )
          .catch((e: unknown) => {
            console.error("[upload] 批量上传异常:", e);
            showToast(
              `上传异常: ${e instanceof Error ? e.message : String(e)}`,
              "error",
            );
          });
      } else {
        console.log("[upload] 当前页对局均已上传过，跳过");
      }
    }
  } catch (e) {
    console.error("抓取战绩列表失败:", e);
    // 首次加载失败时清空残留的上一次查询数据，避免展示归属错误的对局
    if (allMatchesSearch.value.length === 0) {
      matches.value = [];
      resetSelection();
    }
    showToast(`战绩列表获取失败: ${String(e)}`, "error");
  }
}

/** 先试 LCU，重复了降级 SGP */
async function loadMoreMatches() {
  if (!summoner.value || loadingMore.value) return; // 防抖：防止连续触发
  loadingMore.value = true;
  const gen = searchGeneration; // 捕获发起时的搜索代数
  const puuid = summoner.value.puuid;
  try {
    const fetchBeg = loadedGameIndex.value;
    const fetchEnd = fetchBeg + LOAD_MORE_COUNT - 1;

    // 1. 先试 LCU
    let raw = await fetchMatchHistory(puuid, fetchBeg, fetchEnd);
    if (gen !== searchGeneration) return; // 期间发起了新搜索，丢弃旧结果
    console.log(
      `[Search] loadMoreMatches(LCU): beg=${fetchBeg}, end=${fetchEnd}, raw.length=${raw.length}`,
    );

    // 2. 去重检测：所有 gameId 是否都已存在？
    let existingIds = new Set(allMatchesSearch.value.map((m) => m.gameId));
    let newGames = raw.filter((m) => !existingIds.has(m.gameId));

    // 3. LCU 全是重复 → 降级 SGP
    if (newGames.length === 0 && raw.length > 0) {
      console.log(`[Search] LCU 返回全重复，降级 SGP`);
      try {
        raw = await fetchMatchHistorySgp(puuid, fetchBeg, fetchEnd);
        if (gen !== searchGeneration) return; // 期间发起了新搜索，丢弃旧结果
        console.log(
          `[Search] loadMoreMatches(SGP): beg=${fetchBeg}, end=${fetchEnd}, raw.length=${raw.length}`,
        );
        existingIds = new Set(allMatchesSearch.value.map((m) => m.gameId));
        newGames = raw.filter((m) => !existingIds.has(m.gameId));
      } catch (sgpError) {
        console.warn("[Search] SGP 战绩降级拉取失败:", sgpError);
        // SGP 失败时保持 newGames 为空，不中断原有战绩数据流
      }
    }

    // 4. 追加新对局（上限 500 条，防内存泄漏）
    if (newGames.length > 0) {
      allMatchesSearch.value.push(...newGames);
      if (allMatchesSearch.value.length > 500) {
        allMatchesSearch.value = allMatchesSearch.value.slice(0, 500);
      }
      console.log(
        `[Search] 新增${newGames.length}条, 总计${allMatchesSearch.value.length}条`,
      );

      // 若当前页因为之前数据不足导致切片不满 10 条，收到新数据后自动补满当前页
      if (matches.value.length < matchesPerPage) {
        const beg = (currentPageNum.value - 1) * matchesPerPage;
        const end = beg + matchesPerPage;
        matches.value = allFilteredMatches.value.slice(beg, end);
      }
    } else {
      console.log(`[Search] 无新增数据`);
    }

    loadedGameIndex.value = fetchEnd;

    // 后台预取完成后刷新 hasMore，避免翻页按钮状态过期
    const currentEnd = currentPageNum.value * matchesPerPage;
    hasMore.value = allFilteredMatches.value.length > currentEnd;
  } catch (e) {
    console.warn("[Search] 增量加载失败:", e);
  } finally {
    loadingMore.value = false;
  }
}

async function handlePrevPage() {
  if (searching.value || pageSwitching.value) return; // 防重入
  if (currentPageNum.value > 1) {
    pageSwitching.value = true;
    try {
      currentPageNum.value--;
      await loadMatchHistoryList();
      if (matches.value.length > 0) {
        selectMatch(matches.value[0].gameId);
      }
    } finally {
      pageSwitching.value = false;
    }
  }
}

async function handleNextPage() {
  if (!hasMore.value || pageSwitching.value || searching.value) return;
  pageSwitching.value = true;
  try {
    const targetPage = currentPageNum.value + 1;
    const requiredCount = targetPage * matchesPerPage;

    // 如果当前缓存里的数据不够填满下一页，等待增量拉取完成
    while (allFilteredMatches.value.length < requiredCount) {
      const prevCount = allMatchesSearch.value.length;
      await loadMoreMatches();
      // 如果尝试拉取后总数没有增加，说明已经触达接口最底层尽头
      if (allMatchesSearch.value.length === prevCount) {
        break;
      }
    }

    // 如果尝试拉取后依然没有下一页的数据（例如确实打完所有对局了），不翻页
    if (allFilteredMatches.value.length <= (targetPage - 1) * matchesPerPage) {
      hasMore.value = false;
      return;
    }

    currentPageNum.value = targetPage;
    await loadMatchHistoryList();
    if (matches.value.length > 0) {
      selectMatch(matches.value[0].gameId);
    }
  } finally {
    pageSwitching.value = false;
  }
}

function copyGameId(gameId: number) {
  navigator.clipboard.writeText(String(gameId));
  showToast(`游戏 ID: ${gameId} 已复制到剪贴板`);
}

const gameDetails = computed<GameDetail | null>(() => {
  if (!selectedGame.value) return null;
  return buildGameDetail(
    selectedGame.value,
    gameDataAssets.value,
    summoner.value?.puuid || null,
  );
});
</script>

<template>
  <div class="search-view">
    <LcuOfflineState v-if="!store.isConnected" />

    <div v-else class="search-container">
      <!-- 顶部搜索工具栏 -->
      <div class="search-bar">
        <div class="search-input-wrapper">
          <n-input
            v-model:value="searchName"
            :placeholder="t('search.searchPlaceholder')"
            :disabled="searching"
            clearable
            @keyup.enter="doSearch"
            @focus="showHistory = true"
            @click="showHistory = true"
            @blur="hideHistoryDelayed"
            style="width: 100%"
            size="small"
          >
            <template #suffix>
              <n-button
                quaternary
                circle
                size="tiny"
                :disabled="searching || !searchName.trim()"
                @click="doSearch"
              >
                <template #icon>
                  <svg
                    class="search-icon"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <circle cx="11" cy="11" r="8" />
                    <line x1="21" y1="21" x2="16.65" y2="16.65" />
                  </svg>
                </template>
              </n-button>
            </template>
          </n-input>
          <!-- 搜索历史下拉框 -->
          <div
            v-if="showHistory && filteredHistory.length > 0"
            class="history-dropdown"
          >
            <div class="history-header">
              <span class="history-title">🕐 {{ $t("search.history") }}</span>
            </div>
            <div class="history-tags-container">
              <div
                v-for="item in filteredHistory"
                :key="item"
                class="history-tag"
                @mousedown.prevent="selectHistory(item)"
              >
                <span class="history-text" :title="item">{{ item }}</span>
                <span
                  class="history-delete"
                  @mousedown.prevent.stop="removeFromHistory(item)"
                  :title="t('tools.cancel')"
                  >✕</span
                >
              </div>
            </div>
          </div>
        </div>

        <n-button size="small" @click="navigateTo('career')">{{
          $t("nav.career")
        }}</n-button>

        <n-select
          v-model:value="selectedQueue"
          :options="
            QUEUE_OPTIONS.map((q) => ({
              label:
                q.id === null || q.id === -1
                  ? $t('career.all')
                  : $t('gameModes.' + q.id),
              value: q.id === null ? -1 : q.id,
            }))
          "
          @update:value="selectQueue"
          style="width: 130px"
          size="small"
        />

        <n-checkbox
          v-if="appConfig?.General?.UploadApiUrl"
          :checked="uploadEnabled"
          @update:checked="onUploadToggle"
        >
          Upload matches
        </n-checkbox>
      </div>

      <div v-if="error" class="error">{{ error }}</div>

      <!-- 分栏对局面板 -->
      <!-- 分栏对局面板容器 -->
      <div class="panel-layout-container">
        <div class="panel-layout">
          <!-- 左侧：迷你对局卡片列表 -->
          <div class="left-match-list-panel">
            <template v-if="summoner && (matches.length > 0 || currentPageNum > 1)">
              <MiniMatchList
                :matches="matches"
                :selected-game-id="selectedGameId"
                :current-page-num="currentPageNum"
                :has-more="hasMore"
                @select="selectMatch"
                @prev="handlePrevPage"
                @next="handleNextPage"
              />
            </template>
            <!-- 如果没有战绩，左侧展示 10 个等高的骨架空白卡片框 -->
            <template v-else>
              <div class="mini-match-list-skeleton">
                <div
                  v-for="i in 10"
                  :key="i"
                  class="mini-match-card skeleton-card"
                ></div>
              </div>
              <!-- 如果没有数据，渲染一个同等高度的空白骨架翻页占位框 -->
              <div class="pagination-skeleton"></div>
            </template>
          </div>

          <!-- 右侧：对局详情 -->
          <MatchDetailPanel
            :details="gameDetails"
            :loading="gameLoading"
            :queue-id="selectedGame?.queueId ?? null"
            :participant-ranks="participantRanks"
            :my-puuid="summoner?.puuid"
            @copy="copyGameId"
            @search-player="searchPlayerBySummonerId"
          />
        </div>

        <!-- 搜索过程中的高斯模糊半透明遮罩 -->
        <Transition name="fade">
          <div v-if="searching" class="panel-searching-overlay">
            <div class="overlay-glass">
              <n-spin size="large">
                <template #description>
                  <span class="searching-text">{{
                    $t("search.readingMatches")
                  }}</span>
                </template>
              </n-spin>
            </div>
          </div>
        </Transition>
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-view {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}

.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

.error {
  color: var(--loss-color);
  background: var(--loss-bg);
  border: 1px solid var(--loss-border);
  padding: 8px 16px;
  border-radius: 6px;
  margin-bottom: 1rem;
  font-size: 0.82rem;
}

.search-container {
  width: 100%;
  max-width: 1380px;
  box-sizing: border-box;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

/* 顶部搜索栏 */
.search-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  background: var(--card-bg);
  border: 1px solid rgba(0, 0, 0, 0.05);
  padding: 10px 16px;
  border-radius: 8px;
  margin-bottom: 1.2rem;
  box-shadow: var(--shadow-sm);
  position: sticky;
  top: 0;
  z-index: 999 !important;
  width: 100%;
  box-sizing: border-box;
}

.search-input-wrapper {
  position: relative;
  display: flex;
  flex: 1;
  max-width: 580px;
}

/* 搜索历史下拉框 */
.history-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: var(--settings-collapse-bg, var(--card-bg)) !important;
  backdrop-filter: blur(15px) !important;
  border: 1px solid var(--border-color);
  border-top: none;
  border-radius: 0 0 8px 8px;
  box-shadow: var(--shadow-lg);
  z-index: 200;
  max-height: 260px;
  overflow-y: auto;
  padding: 12px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px dashed var(--border-color);
  padding-bottom: 6px;
  user-select: none;
}

.history-title {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.5px;
}

.history-tags-container {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.history-tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  cursor: pointer;
  transition: all 0.2s ease-in-out;
  max-width: 260px;
  box-sizing: border-box;
}

.history-tag:hover {
  background: var(--primary-color-alpha-10);
  border-color: rgba(var(--primary-color-rgb, 59, 130, 246), 0.3);
  transform: translateY(-1px);
}

.history-tag:hover .history-text {
  color: var(--primary-color);
}

.history-text {
  font-size: 0.78rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 600;
  transition: color 0.2s ease-in-out;
}

.history-delete {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  font-size: 0.62rem;
  color: var(--text-dimmed);
  border-radius: 50%;
  transition: all 0.15s ease-in-out;
  flex-shrink: 0;
}

.history-delete:hover {
  background: var(--loss-bg);
  color: var(--loss-color);
  transform: scale(1.1);
}

.search-input {
  width: 100%;
  padding: 6px 36px 6px 12px;
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  border-radius: 6px;
  font-size: 0.85rem;
  color: var(--text-color);
  outline: none;
  transition: all 0.2s;
  text-align: center;
  height: 32px;
}

.search-input:focus {
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-15);
  background: var(--card-bg);
}

.search-trigger-btn {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  padding: 4px;
}

.search-icon {
  width: 16px;
  height: 16px;
}

.tab-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 0 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  height: 32px;
  display: inline-flex;
  align-items: center;
}

.tab-btn:hover {
  background: var(--card-bg);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.tab-btn.active {
  background-color: var(--card-bg);
  color: var(--text-color);
  border-color: var(--border-color);
  font-weight: 600;
  box-shadow: none;
}

.tab-btn.active:hover {
  background: var(--card-bg);
  border-color: var(--primary-color);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 0.82rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  height: 32px;
}

.dropdown-trigger:hover {
  background: var(--card-bg);
  border-color: var(--primary-color);
}

.dropdown-trigger .arrow-icon {
  width: 12px;
  height: 12px;
  transition: transform 0.2s;
}

.dropdown-trigger .arrow-icon.expanded {
  transform: rotate(180deg);
}

.queue-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 130px;
  padding: 4px 0;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}

.queue-dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.queue-dropdown-item:hover {
  background: rgba(0, 0, 0, 0.02);
  color: var(--text-color);
}

.queue-dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.checkbox-wrapper {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 0.82rem;
  color: var(--text-muted);
  cursor: pointer;
}

/* 分栏大布局 */
.panel-layout {
  display: grid;
  grid-template-columns: 180px 1fr;
  gap: 16px;
  align-items: stretch;
  animation: fadeInUp 0.45s cubic-bezier(0.25, 0.8, 0.25, 1) forwards;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 战绩面板过渡与搜索遮罩 */
.panel-layout-container {
  position: relative;
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.panel-searching-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(255, 255, 255, 0.25);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  border-radius: 12px;
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

[data-theme="dark"] .panel-searching-overlay {
  background: rgba(10, 10, 15, 0.45);
}

.overlay-glass {
  background: rgba(255, 255, 255, 0.65);
  padding: 24px 40px;
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.4);
  box-shadow:
    0 16px 36px rgba(31, 38, 135, 0.08),
    0 0 0 1px rgba(255, 255, 255, 0.1) inset;
  display: flex;
  align-items: center;
  justify-content: center;
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}

[data-theme="dark"] .overlay-glass {
  background: rgba(30, 30, 45, 0.7);
  border-color: rgba(255, 255, 255, 0.08);
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.35);
}

.searching-text {
  color: var(--text-color);
  font-size: 0.88rem;
  font-weight: 600;
  margin-top: 8px;
}

.searching-welcome-text {
  color: var(--text-color);
  font-size: 0.92rem;
  font-weight: 600;
}

/* 首次欢迎与加载占位 */
.mini-match-list-skeleton {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  overflow: hidden;
}

.mini-match-card.skeleton-card {
  height: 58px;
  box-sizing: border-box;
  border-radius: 8px;
  cursor: default;
  pointer-events: none;
  background: rgba(0, 0, 0, 0.015);
  border: 1px dashed var(--border-color);
  box-shadow: none;
}

.pagination-skeleton {
  height: 38px;
  margin-top: 10px;
  box-sizing: border-box;
}

[data-theme="dark"] .mini-match-card.skeleton-card {
  background: rgba(255, 255, 255, 0.01);
}

@keyframes float {
  0%,
  100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-8px);
  }
}

@keyframes fadeInUp {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
