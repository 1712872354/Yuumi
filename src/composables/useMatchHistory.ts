import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  fetchCurrentSummoner,
  fetchSummonerByPuuid,
  fetchMatchHistorySmart,
} from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import type { RankDisplaySource, RankedQueueEntry } from "../types/lcu";
import { lazySetItem } from "../utils/lazyStorage";
import { QUEUE_FILTER_OPTIONS, formatRankDisplay } from "../utils/queueMeta";
import { getQueueName as resolveQueueName } from "../utils/queueName";
import { computeStatsSummary } from "./gamePlayerStats";
import { mergeMatchLists } from "./gameMatchesCache";
import { fetchRankedStatsCached } from "./playerMastery";

// 模块作用域内存缓存单例
let cachedSummoner: SummonerDisplay | null = null;
let cachedMatches: MatchDisplay[] = [];
let cachedRecentMatches: MatchDisplay[] = [];
let cachedRankedQueues: RankedQueueEntry[] = [];
let lastFetchedTime = 0;

export function useMatchHistory() {
  const { t, te } = useI18n();

  const summoner = ref<SummonerDisplay | null>(null);
  const currentLoginPuuid = ref<string>("");
  const isViewingOther = computed(() => {
    return (
      !!currentLoginPuuid.value &&
      !!summoner.value?.puuid &&
      summoner.value.puuid !== currentLoginPuuid.value
    );
  });
  const matches = ref<MatchDisplay[]>([]);
  const recentMatches = ref<MatchDisplay[]>([]);
  const rankedQueues = ref<RankedQueueEntry[]>([]);
  const loading = ref(false);
  const error = ref("");
  const copied = ref(false);
  const careerGamesNumber = ref(20);

  // 游戏模式筛选
  const selectedQueue = ref<number | null>(null);
  const QUEUE_OPTIONS = QUEUE_FILTER_OPTIONS;
  const showQueueDropdown = ref(false);

  // 计算属性
  const filteredMatches = computed(() => {
    if (selectedQueue.value === null) return matches.value;
    return matches.value.filter(
      (m: MatchDisplay) => m.queueId === selectedQueue.value,
    );
  });

  const soloQueue = computed(() => {
    return rankedQueues.value.find((q) => q.queueType === "RANKED_SOLO_5x5") || null;
  });

  const flexQueue = computed(() => {
    return rankedQueues.value.find((q) => q.queueType === "RANKED_FLEX_SR") || null;
  });

  const statsSummary = computed(() => computeStatsSummary(recentMatches.value));

  // ─── 数据加载 ───

  const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_matches_cache_${puuid}`;

  async function fetchMatchHistoryWithFallback(
    puuid: string,
    begIndex: number,
    endIndex: number,
    isGameEndSync = false,
  ): Promise<MatchDisplay[]> {
    return fetchMatchHistorySmart(puuid, begIndex, endIndex, {
      forceSgp: isGameEndSync,
    });
  }

  async function loadSummoner(forceRefresh = false) {
    const now = Date.now();
    if (!forceRefresh && cachedSummoner && now - lastFetchedTime < 20000) {
      summoner.value = cachedSummoner;
      currentLoginPuuid.value = cachedSummoner.puuid;
      rankedQueues.value = cachedRankedQueues;
      matches.value = cachedMatches;
      recentMatches.value = cachedRecentMatches;
      return;
    }

    loading.value = true;
    error.value = "";
    try {
      summoner.value = await fetchCurrentSummoner();
      if (summoner.value?.puuid) {
        currentLoginPuuid.value = summoner.value.puuid;
        await Promise.all([
          loadRankedStats(summoner.value.puuid),
          loadCareerData(summoner.value.puuid),
        ]);

        cachedSummoner = summoner.value;
        cachedRankedQueues = rankedQueues.value;
        cachedMatches = matches.value;
        cachedRecentMatches = recentMatches.value;
        lastFetchedTime = Date.now();
      }
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }

  async function loadCareerSummoner(targetPuuid?: string, forceRefresh = false) {
    if (!targetPuuid || (currentLoginPuuid.value && targetPuuid === currentLoginPuuid.value)) {
      await loadSummoner(forceRefresh);
      return;
    }

    loading.value = true;
    error.value = "";
    try {
      const targetSummoner = await fetchSummonerByPuuid(targetPuuid);
      if (!targetSummoner) {
        throw new Error("获取召唤师信息失败");
      }
      summoner.value = targetSummoner;
      await Promise.all([
        loadRankedStats(targetPuuid),
        loadCareerData(targetPuuid),
      ]);
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }

  async function backToMyCareer() {
    await loadSummoner(false);
  }

  async function loadRankedStats(puuid: string) {
    try {
      const resp = await fetchRankedStatsCached(puuid);
      if (resp.success && resp.data?.queues) {
        rankedQueues.value = resp.data.queues;
      }
    } catch (e) {
      console.error("获取排位段位数据失败:", e);
    }
  }

  // 对局结束后仅刷新召唤师头部数据（等级等），不重复拉取战绩。
  // 战绩刷新由 MatchHistoryTab 的重试逻辑统一负责，避免双重请求。
  // 注意：不更新 lastFetchedTime——该时间戳表示「整包缓存」的新鲜度，
  // 只更新 summoner 时抬高会让后续 loadSummoner(false) 在 20s 内返回过期战绩。
  async function refreshSummonerOnly() {
    if (isViewingOther.value) return;
    try {
      summoner.value = await fetchCurrentSummoner();
      if (summoner.value?.puuid) {
        currentLoginPuuid.value = summoner.value.puuid;
        cachedSummoner = summoner.value;
      }
    } catch (e) {
      console.error("刷新召唤师数据失败:", e);
    }
  }

  // 一次拉取战绩：matches 与 recentMatches 共用同一份数据，避免重复请求同一接口
  async function loadCareerData(puuid: string, isGameEndSync = false) {
    try {
      const targetCount = careerGamesNumber.value;
      const raw = await fetchMatchHistoryWithFallback(
        puuid, 0, targetCount, isGameEndSync,
      );
      matches.value = raw.slice(0, targetCount);
      updateRecentMatchesCache(puuid, raw);
    } catch (e) {
      console.error("获取战绩历史失败:", e);
    }
  }

  function updateRecentMatchesCache(puuid: string, fresh: MatchDisplay[]) {
    let cached: MatchDisplay[] = [];
    try {
      const raw = localStorage.getItem(MATCHES_CACHE_KEY(puuid));
      if (raw) cached = JSON.parse(raw);
    } catch { /* ignore */ }

    const merged = mergeMatchLists(
      fresh,
      cached,
      careerGamesNumber.value,
    );
    recentMatches.value = merged;
    lazySetItem(MATCHES_CACHE_KEY(puuid), merged);
  }

  // ─── 辅助函数 ───

  function selectQueue(id: number | null) {
    selectedQueue.value = id;
    showQueueDropdown.value = false;
  }

  function formatRank(queue: RankDisplaySource | null) {
    if (!queue) return "--";
    const raw = queue.rank && queue.rank !== "NA" ? queue.rank : queue.division;
    return formatRankDisplay(queue.tier, raw);
  }

  function formatHighestRank(queue: RankDisplaySource | null) {
    if (!queue) return "--";
    return formatRankDisplay(queue.highestTier, queue.highestRank);
  }

  function formatPrevSeasonRank(queue: RankDisplaySource | null) {
    if (!queue) return "--";
    return formatRankDisplay(queue.previousSeasonEndTier, queue.previousSeasonEndRank);
  }

  async function copyRiotId() {
    if (!summoner.value) return;
    const fullId = `${summoner.value.gameName || summoner.value.displayName}#${summoner.value.tagLine}`;
    try {
      await navigator.clipboard.writeText(fullId);
      copied.value = true;
      setTimeout(() => { copied.value = false; }, 1500);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = fullId;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      copied.value = true;
      setTimeout(() => { copied.value = false; }, 1500);
    }
  }

  function getSpellIcon(m: MatchDisplay, slot: 1 | 2): string {
    return slot === 1 ? m.spell1IconUrl : m.spell2IconUrl;
  }

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

  function getKdaClass(kda: string): string {
    const val = parseFloat(kda);
    if (isNaN(val)) return "kda-perfect";
    if (val >= 5) return "kda-great";
    if (val >= 3) return "kda-good";
    return "kda-normal";
  }

  // 清除缓存
  function clearCache() {
    cachedSummoner = null;
    cachedMatches = [];
    cachedRecentMatches = [];
    cachedRankedQueues = [];
    lastFetchedTime = 0;
  }

  return {
    // 状态
    summoner,
    currentLoginPuuid,
    isViewingOther,
    matches,
    recentMatches,
    rankedQueues,
    loading,
    error,
    copied,
    careerGamesNumber,
    selectedQueue,
    QUEUE_OPTIONS,
    showQueueDropdown,
    filteredMatches,
    soloQueue,
    flexQueue,
    statsSummary,

    // 方法
    loadSummoner,
    loadCareerSummoner,
    backToMyCareer,
    loadCareerData,
    loadRankedStats,
    refreshSummonerOnly,
    selectQueue,
    formatRank,
    formatHighestRank,
    formatPrevSeasonRank,
    copyRiotId,
    getSpellIcon,
    formatTime,
    translateMapName,
    getQueueName,
    getKdaClass,
    clearCache,
    fetchMatchHistoryWithFallback,
  };
}
