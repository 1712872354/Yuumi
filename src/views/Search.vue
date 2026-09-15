<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, inject, watch, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import {
  fetchCurrentSummoner,
  fetchConfig,
  updateConfig,
  type AppConfig,
} from "../api/lcu";
import type { GameDataAssets } from "../types/lcu";
import LcuOfflineState from "../components/LcuOfflineState.vue";
import MiniMatchList from "../components/search/MiniMatchList.vue";
import MatchDetailPanel from "../components/search/MatchDetailPanel.vue";
import SearchToolbar from "../components/search/SearchToolbar.vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useToast } from "../composables/useToast";
import type { GameDetail } from "../types/search";
import { buildGameDetail } from "../utils/gameDetailBuilder";
import { useMatchDetailCache } from "../composables/useMatchDetailCache";
import { useMatchSearchPage } from "../composables/useMatchSearchPage";

const store = useLcuStore();
const navigateTo = inject<(page: string) => void>("navigateTo")!;
const { showToast } = useToast();

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

const search = useMatchSearchPage({
  selectMatch,
  resetSelection,
});

const {
  searchName,
  searching,
  error,
  summoner,
  matches,
  selectedQueue,
  uploadEnabled,
  currentPageNum,
  hasMore,
  showHistory,
  filteredHistory,
  loadSearchHistory,
  removeFromHistory,
  hideHistoryDelayed,
  doSearch,
  waitForSearchIdle,
  handlePrevPage,
  handleNextPage,
  selectQueue,
  selectHistoryItem,
  searchPlayerBySummonerId,
} = search;

const showUploadCheckbox = computed(() => !!appConfig.value?.General?.UploadApiUrl);

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
      /* ignore */
    }
  },
);

watch(
  navigateSearchPayload,
  async (payload) => {
    if (!payload || !payload.name || payload.gameId === null) return;
    searchName.value = payload.name;
    let started = await doSearch();
    while (!started) {
      if (navigateSearchPayload.value !== payload) return;
      const idle = await waitForSearchIdle();
      if (!idle) continue;
      if (navigateSearchPayload.value !== payload) return;
      searchName.value = payload.name;
      started = await doSearch();
    }
    if (payload.gameId > 0 && matches.value.length > 0) {
      await selectMatch(payload.gameId);
    }
    navigateSearchPayload.value = null;
  },
  { immediate: true },
);
</script>

<template>
  <div class="search-view">
    <LcuOfflineState v-if="!store.isConnected" />

    <div v-else class="search-container">
      <SearchToolbar
        v-model:search-name="searchName"
        v-model:selected-queue="selectedQueue"
        :searching="searching"
        :upload-enabled="uploadEnabled"
        :show-upload-checkbox="showUploadCheckbox"
        :show-history="showHistory"
        :filtered-history="filteredHistory"
        @search="doSearch"
        @select-history="selectHistoryItem"
        @remove-history="removeFromHistory"
        @hide-history="hideHistoryDelayed"
        @show-history-panel="showHistory = true"
        @update:selected-queue="selectQueue"
        @update:upload-enabled="onUploadToggle"
        @go-career="navigateTo('career')"
      />

      <div v-if="error" class="error">{{ error }}</div>

      <div class="panel-layout-container">
        <div class="panel-layout">
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
            <template v-else>
              <div class="mini-match-list-skeleton">
                <div
                  v-for="i in 10"
                  :key="i"
                  class="mini-match-card skeleton-card"
                ></div>
              </div>
              <div class="pagination-skeleton"></div>
            </template>
          </div>

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
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  height: 100%;
}
.search-container {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  height: 100%;
}
.error {
  color: #dc2626;
  font-size: 13px;
  margin-bottom: 8px;
}
.panel-layout-container {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.panel-layout {
  display: flex;
  gap: 12px;
  flex: 1;
  min-height: 0;
}
.left-match-list-panel {
  display: flex;
  flex-direction: column;
  width: 320px;
  flex-shrink: 0;
  min-height: 0;
}
.mini-match-list-skeleton {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
}
.skeleton-card {
  height: 56px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.04);
}
.pagination-skeleton {
  height: 36px;
  margin-top: 8px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.03);
}
.panel-searching-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
}
.overlay-glass {
  padding: 24px 32px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.55);
  backdrop-filter: blur(10px);
}
.searching-text {
  color: var(--text-muted, #6b7280);
  font-size: 13px;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
