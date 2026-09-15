<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, inject, type Ref } from "vue";
import { useLcuStore } from "../../store/lcuStore";
import { fetchRecentTeammates, fetchConfig } from "../../api/lcu";
import type { SummonerDisplay, MatchDisplay, RecentTeammate } from "../../api/lcu";
import type { RankedQueueEntry } from "../../types/lcu";
import LcuImage from "../LcuImage.vue";
import { NPopover, NSpin } from "naive-ui";
import { QUEUE_FILTER_OPTIONS } from "../../utils/queueMeta";
import { computeStatsSummary } from "../../composables/gamePlayerStats";
import { useMatchDisplayHelpers } from "../../composables/matchDisplayHelpers";
import MatchRankTable from "./MatchRankTable.vue";
import MatchHistoryCard from "./MatchHistoryCard.vue";

const store = useLcuStore();
const { formatTime } = useMatchDisplayHelpers();

// 从 Career.vue inject 共享的 composable 状态（单一实例）
const mh = inject<{
  summoner: Ref<SummonerDisplay | null>;
  matches: Ref<MatchDisplay[]>;
  recentMatches: Ref<MatchDisplay[]>;
  rankedQueues: Ref<RankedQueueEntry[]>;
  loading: Ref<boolean>;
  isViewingOther: Ref<boolean>;
  loadSummoner: (force?: boolean) => Promise<void>;
  loadCareerSummoner: (puuid: string, force?: boolean) => Promise<void>;
  backToMyCareer: () => Promise<void>;
  loadCareerData: (puuid: string, sync?: boolean) => Promise<void>;
  loadRankedStats: (puuid: string) => Promise<void>;
  clearCache: () => void;
  fetchMatchHistoryWithFallback: (puuid: string, beg: number, end: number, sync?: boolean) => Promise<MatchDisplay[]>;
}>("matchHistoryState")!;

const { summoner, matches, recentMatches, rankedQueues, loading, loadCareerSummoner } = mh;

// ─── 本地 UI 状态（仅本组件使用）───
const careerGamesNumber = ref(20);
const selectedQueue = ref<number | null>(null);
const showQueueDropdown = ref(false);
const loadingTeammates = ref(false);
const recentTeammates = ref<RecentTeammate[]>([]);
let currentTeammatePuuid = "";
let lastCalculatedGameIds = "";

const QUEUE_OPTIONS = QUEUE_FILTER_OPTIONS;

// ─── 计算属性 ───
const filteredMatches = computed(() => {
  if (selectedQueue.value === null) return matches.value;
  return matches.value.filter((m: MatchDisplay) => m.queueId === selectedQueue.value);
});

const soloQueue = computed(() =>
  rankedQueues.value.find((q) => q.queueType === "RANKED_SOLO_5x5") || null,
);

const flexQueue = computed(() =>
  rankedQueues.value.find((q) => q.queueType === "RANKED_FLEX_SR") || null,
);

const statsSummary = computed(() => computeStatsSummary(recentMatches.value));

// ─── 辅助函数 ───
function selectQueue(id: number | null) {
  selectedQueue.value = id;
  showQueueDropdown.value = false;
}

async function calculateRecentTeammates() {
  if (!summoner.value?.puuid || recentMatches.value.length === 0) {
    recentTeammates.value = [];
    lastCalculatedGameIds = "";
    return;
  }
  const targetPuuid = summoner.value.puuid;
  const gameIds = recentMatches.value.map((m) => m.gameId);
  const gameIdsKey = `${targetPuuid}_${gameIds.join(",")}`;

  currentTeammatePuuid = targetPuuid;
  loadingTeammates.value = true;
  try {
    const resp = await fetchRecentTeammates(gameIds, targetPuuid);
    if (currentTeammatePuuid === targetPuuid) {
      recentTeammates.value = resp.summoners || [];
      lastCalculatedGameIds = gameIdsKey;
    }
  } catch (err) {
    console.error("计算最近队友失败:", err);
    if (currentTeammatePuuid === targetPuuid) {
      recentTeammates.value = [];
    }
  } finally {
    if (currentTeammatePuuid === targetPuuid) {
      loadingTeammates.value = false;
    }
  }
}

function handleTeammatesPopoverShow(show: boolean) {
  if (!show) return;
  const currentKey = summoner.value?.puuid
    ? `${summoner.value.puuid}_${recentMatches.value.map((m) => m.gameId).join(",")}`
    : "";
  if (
    !loadingTeammates.value &&
    (recentTeammates.value.length === 0 || lastCalculatedGameIds !== currentKey)
  ) {
    calculateRecentTeammates();
  }
}

// ─── 导航函数 ───
const navigateSearchPayload = inject<
  Ref<{ name: string; gameId: number | null } | null>
>("navigateSearchPayload")!;
const navigateTo = inject<(page: string) => void>("navigateTo");

function goToMatchDetail(gameId: number) {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  navigateSearchPayload.value = { name: fullName, gameId };
}

function handleClickTeammate(tm: RecentTeammate) {
  if (tm.puuid) {
    loadCareerSummoner(tm.puuid);
    return;
  }
  if (navigateSearchPayload) {
    navigateSearchPayload.value = { name: tm.name, gameId: -1 };
  }
  if (navigateTo) {
    navigateTo("search");
  }
}

// 点击外部关闭下拉菜单
function onDocClick() {
  showQueueDropdown.value = false;
}

// ─── 生命周期与监听 ───

onMounted(async () => {
  try {
    const cfg = await fetchConfig();
    careerGamesNumber.value = cfg.Functions?.CareerGamesNumber ?? 20;
  } catch (e) {
    console.warn("加载 CareerGamesNumber 配置失败，使用默认值 20:", e);
  }
  document.addEventListener("click", onDocClick);
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
});

// 召唤师变更时清空队友缓存
watch(
  () => summoner.value?.puuid,
  (newPuuid, oldPuuid) => {
    if (newPuuid !== oldPuuid) {
      recentTeammates.value = [];
      lastCalculatedGameIds = "";
    }
  },
);

// 对局结束后或新对局开始时自动刷新战绩列表（召唤师数据由 Career.vue 统一刷新）
let isGameEndSyncing = false;
watch(
  () => store.gameEndedTrigger,
  async (trigger) => {
    if (!trigger || !summoner.value?.puuid || isGameEndSyncing) return;
    isGameEndSyncing = true;
    const puuid = summoner.value.puuid;
    const prevLatestId =
      recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
    console.log(
      `[Career] 对局结束信号触发，开始快速拉取最新战绩，当前最新 gameId:`,
      prevLatestId,
    );

    try {
      // 立即尝试首次拉取（SGP 权威接口通常在结算时已存在该对局）
      await mh.loadCareerData(puuid, true);
      const firstId = matches.value[0]?.gameId ?? null;
      if (firstId && firstId !== prevLatestId) {
        console.log(`[Career] 立即同步成功，已更新最新对局 ${firstId}`);
        await calculateRecentTeammates();
        return;
      }

      // 若首次拉取未刷新，等待 1.5 秒后以更密集间隔（1.5s、2s、3s、3s）重试
      const retryDelays = [1500, 2000, 3000, 3000];
      for (let attempt = 0; attempt < retryDelays.length; attempt++) {
        await new Promise((r) => setTimeout(r, retryDelays[attempt]));
        try {
          await mh.loadCareerData(puuid, true);
          const latestId = matches.value[0]?.gameId ?? null;
          if (latestId && latestId !== prevLatestId) {
            console.log(
              `[Career] 第 ${attempt + 1} 次重试已检测到新对局 ${latestId}`,
            );
            await calculateRecentTeammates();
            return;
          }
          console.log(
            `[Career] 第 ${attempt + 1} 次重试尚未发现新对局，继续等待`,
          );
        } catch (e) {
          console.warn(`[Career] 第 ${attempt + 1} 次刷新重试失败:`, e);
        }
      }
      await mh.loadRankedStats(puuid);
      await calculateRecentTeammates();
    } finally {
      isGameEndSyncing = false;
    }
  },
);

// 新对局开始时（例如进入BP阶段），重置缓存并拉取最新战绩，避免仍停留在上一把更早的战绩
watch(
  () => store.newGameStartedTrigger,
  async (trigger) => {
    if (!trigger || !summoner.value?.puuid) return;
    console.log("[Career] 检测到新对局开始，静默刷新最新战绩");
    try {
      await mh.loadCareerData(summoner.value.puuid, false);
      await calculateRecentTeammates();
    } catch (e) {
      console.warn("[Career] 新对局静默刷新战绩失败:", e);
    }
  },
);
</script>

<template>
  <div class="match-history-tab">
    <!-- 加载中：召唤师数据尚未就绪 -->
    <div v-if="!summoner && loading" class="tip-container">
      <n-spin size="large" />
      <p class="tip" style="margin-top: 1rem;">{{ $t("career.loading") || "加载中..." }}</p>
    </div>

    <!-- 空态：加载完成但无数据 -->
    <div v-else-if="!summoner" class="tip-container">
      <p class="tip">{{ $t("career.empty") }}</p>
    </div>

    <template v-else>
    <MatchRankTable :solo-queue="soloQueue" :flex-queue="flexQueue" />

    <!-- 近期数据看板 & 常用英雄 -->
    <div v-if="statsSummary" class="recent-summary-bar">
      <div class="summary-text">
        <span class="summary-title">{{
          $t("career.recentGamesTitle", { count: recentMatches.length })
        }}</span>
        <span class="win-color"
          >{{ $t("career.win") }}: {{ statsSummary.wins }}</span
        >
        <span class="lose-color"
          >{{ $t("career.lose") }}: {{ statsSummary.losses }}</span
        >
        <span class="kda-label">KDA:</span>
        <span class="kda-values">
          {{ statsSummary.kills }} /
          <span class="death-red">{{ statsSummary.deaths }}</span> /
          {{ statsSummary.assists }}
        </span>
        <span class="kda-ratio">({{ statsSummary.kda }})</span>
      </div>

      <div class="recent-champs">
        <div
          v-for="c in statsSummary.topChamps"
          :key="c.id"
          class="recent-champ-icon"
          :title="$t('career.gamesCount', { count: c.count })"
        >
          <LcuImage :src="c.icon" alt="champ" />
        </div>
      </div>

      <div class="summary-actions">
        <n-popover
          trigger="click"
          placement="bottom-start"
          scrollable
          class="recent-teammates-popover"
          @update:show="handleTeammatesPopoverShow"
          style="
            max-height: 400px;
            width: 420px;
            border-radius: 12px;
            padding: 12px;
          "
        >
          <template #trigger>
            <button class="summary-action-btn">
              {{ $t("career.recentTeammates") }}
            </button>
          </template>
          <div class="teammates-flyout">
            <div v-if="loadingTeammates" class="loading-container">
              <n-spin size="medium" />
            </div>
            <div v-else-if="recentTeammates.length === 0" class="empty-text">
              暂无最近队友数据
            </div>
            <div v-else class="teammates-list">
              <div
                v-for="tm in recentTeammates"
                :key="tm.puuid"
                class="teammate-card"
                @click="handleClickTeammate(tm)"
              >
                <div class="teammate-avatar">
                  <LcuImage :src="tm.icon" />
                </div>
                <div class="teammate-info-col">
                  <div class="teammate-name">
                    {{ tm.name }}
                    <span v-if="tm.tag" class="teammate-tag-badge">{{ tm.tag }}</span>
                  </div>
                  <div class="teammate-last-time" v-if="tm.lastPlayTime">
                    {{ formatTime(tm.lastPlayTime) }}
                  </div>
                </div>
                <div class="teammate-stats">
                  <span class="stat-item">
                    {{ $t("career.total") || "总" }}: <span class="stat-value total">{{ tm.total }}</span>
                  </span>
                  <span class="stat-item">
                    {{ $t("career.win") || "胜" }}: <span class="stat-value win">{{ tm.wins }}</span>
                  </span>
                  <span class="stat-item">
                    {{ $t("career.lose") || "负" }}: <span class="stat-value lose">{{ tm.losses }}</span>
                  </span>
                </div>
              </div>
            </div>
          </div>
        </n-popover>
        <div
          class="dropdown-trigger"
          @click.stop="showQueueDropdown = !showQueueDropdown"
        >
          <span>{{
            selectedQueue === null
              ? $t("career.all")
              : $t("gameModes." + selectedQueue)
          }}</span>
          <svg
            :class="['arrow-icon', { expanded: showQueueDropdown }]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <polyline points="6 9 12 15 18 9" />
          </svg>
          <div
            v-if="showQueueDropdown"
            class="queue-dropdown-menu"
            @click.stop
          >
            <div
              v-for="q in QUEUE_OPTIONS"
              :key="q.id ?? -1"
              :class="[
                'queue-dropdown-item',
                { active: selectedQueue === q.id },
              ]"
              @click="selectQueue(q.id)"
            >
              {{ q.id === null ? $t("career.all") : $t("gameModes." + q.id) }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 局部滚动包裹区域：保留头像、排位与近期对局看板，仅滚动对局战绩列表 -->
    <div class="career-scroll-area">
      <!-- 战绩对局历史列表 -->
      <div v-if="filteredMatches.length > 0" class="match-history-list">
        <MatchHistoryCard
          v-for="m in filteredMatches"
          :key="m.gameId"
          :match="m"
          @click="goToMatchDetail"
        />
      </div>
      <div v-else-if="!loading" class="tip-container">
        <p class="tip">{{ $t("career.empty") }}</p>
      </div>
    </div>
    </template>
  </div>
</template>

<style scoped>
.match-history-tab {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-height: 0;
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

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 近期数据概览栏 */
.recent-summary-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background-color: var(--settings-collapse-bg, var(--card-bg)) !important;
  backdrop-filter: blur(15px) !important;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
  position: sticky;
  top: 0;
  z-index: 100 !important;
}

.recent-summary-bar:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.summary-text {
  font-size: 0.82rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-title {
  font-weight: bold;
  color: var(--text-color);
  margin-right: 4px;
}

.win-color {
  color: var(--win-color);
  font-weight: 600;
}

.lose-color {
  color: var(--loss-color);
  font-weight: 600;
}

.kda-label {
  color: var(--text-dimmed);
  margin-left: 8px;
}

.kda-values {
  color: var(--text-color);
  font-weight: 600;
}

.death-red {
  color: var(--death-color, var(--loss-color));
}

.recent-champs {
  display: flex;
  gap: 4px;
}

.recent-champ-icon {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.summary-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.summary-action-btn:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 0.78rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
}

.dropdown-trigger:hover {
  background: var(--card-bg-hover);
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

/* 模式筛选下拉菜单 */
.queue-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  background: #ffffff;
  border: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 130px;
  padding: 4px 0;
}

[data-theme="dark"] .queue-dropdown-menu {
  background: #18181c;
  border-color: rgba(255, 255, 255, 0.08);
}

.queue-dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.queue-dropdown-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}

.queue-dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

/* 战绩及看板局部滚动区域 */
.career-scroll-area {
  flex: 1;
  overflow-y: auto;

  padding-right: 4px;
  margin-top: 1rem;
  scroll-behavior: smooth;
  /* 顶部与底部边缘 16px 渐变淡出羽化，消除滑动硬切裂感，极致顺滑 */
  -webkit-mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 16px,
    black calc(100% - 16px),
    transparent 100%
  );
  mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 16px,
    black calc(100% - 16px),
    transparent 100%
  );
}

/* 队友弹窗内加载容器 */
.loading-container {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem;
}

.empty-text {
  text-align: center;
  padding: 1rem;
  color: var(--text-dimmed);
  font-size: 0.85rem;
}

/* 队友卡片 */
.teammates-flyout {
  min-width: 380px;
}

.teammates-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.teammate-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.teammate-card:hover {
  background-color: var(--hover-bg);
}

.teammate-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
}

.teammate-info-col {
  flex: 1;
  min-width: 0;
}

.teammate-name {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.teammate-tag-badge {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  font-size: 0.68rem;
  font-weight: 500;
  line-height: 1.4;
  color: #fff;
  background: linear-gradient(135deg, var(--primary-color, #6fa8ff), #9b6fff);
  border-radius: 8px;
  vertical-align: middle;
}

.teammate-last-time {
  font-size: 0.72rem;
  color: var(--text-dimmed);
  margin-top: 2px;
}

.teammate-stats {
  display: flex;
  gap: 10px;
  font-size: 0.78rem;
  color: var(--text-muted);
  flex-shrink: 0;
}

.stat-item {
  white-space: nowrap;
}

.stat-value {
  font-weight: 700;
}

.stat-value.total {
  color: var(--text-color);
}

.stat-value.win {
  color: var(--win-color);
}

.stat-value.lose {
  color: var(--loss-color);
}

/* 海克斯强化悬浮提示 */
.career-aug-tooltip {
  max-width: 280px;
  padding: 8px 10px;
  text-align: left;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-md);
}

.career-aug-tooltip-name {
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--text-color);
  letter-spacing: 0.2px;
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
</style>
