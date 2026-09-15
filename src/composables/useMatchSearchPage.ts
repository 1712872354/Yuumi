/** 战绩搜索页编排：搜索 + 本地翻页 + LCU/SGP 增量加载 + 上传 */
import { computed, ref } from "vue";
import {
  fetchMatchHistory,
  fetchMatchHistorySgp,
  fetchMatchHistorySmart,
  lcuRequest,
  batchUploadMatches,
  type MatchDisplay,
  type SummonerDisplay,
} from "../api/lcu";
import type { RawSummoner } from "../types/lcu";
import { useToast } from "./useToast";
import { useSearchHistory } from "./useSearchHistory";

export interface MatchSearchDeps {
  selectMatch: (gameId: number) => Promise<void> | void;
  resetSelection: () => void;
}

export function useMatchSearchPage(deps: MatchSearchDeps) {
  const { showToast } = useToast();
  const { selectMatch, resetSelection } = deps;

  const searchName = ref("");
  const searching = ref(false);
  const error = ref("");
  const summoner = ref<SummonerDisplay | null>(null);
  const matches = ref<MatchDisplay[]>([]);
  const selectedQueue = ref<number>(-1);
  const uploadEnabled = ref(true);
  const uploadedGameIds = ref(new Set<number>());
  const pendingSummonerId = ref<number>(0);

  const currentPageNum = ref(1);
  const matchesPerPage = 10;
  const hasMore = ref(false);
  const allMatchesSearch = ref<MatchDisplay[]>([]);
  const loadedGameIndex = ref(0);
  const loadingMore = ref(false);
  const pageSwitching = ref(false);
  const INITIAL_BATCH = 20;
  const LOAD_MORE_COUNT = 30;
  const PREFETCH_PAGES = 1;
  let searchGeneration = 0;

  const currentRiotId = computed(() => {
    if (!summoner.value) return "";
    const gn = summoner.value.gameName || summoner.value.displayName;
    const tl = summoner.value.tagLine;
    return tl ? `${gn}#${tl}` : gn;
  });

  const allFilteredMatches = computed(() => {
    if (selectedQueue.value === -1) return allMatchesSearch.value;
    return allMatchesSearch.value.filter(
      (m: MatchDisplay) => m.queueId === selectedQueue.value,
    );
  });

  const {
    showHistory,
    filteredHistory,
    loadSearchHistory,
    saveToHistory,
    removeFromHistory,
    hideHistoryDelayed,
  } = useSearchHistory(searchName, currentRiotId);

  function searchPlayerBySummonerId(summonerId: number, displayName: string) {
    if (!summonerId) return;
    pendingSummonerId.value = summonerId;
    searchName.value = displayName || String(summonerId);
    resetSelection();
    void doSearch();
  }

  async function waitForSearchIdle(timeoutMs = 30000): Promise<boolean> {
    const start = Date.now();
    while (searching.value && Date.now() - start < timeoutMs) {
      await new Promise((r) => setTimeout(r, 50));
    }
    return !searching.value;
  }

  async function doSearch(): Promise<boolean> {
    if (!searchName.value.trim()) return false;
    if (searching.value) return false;
    searching.value = true;
    error.value = "";
    searchGeneration++;

    try {
      const name = searchName.value.trim();
      let resp;
      const summonerId = pendingSummonerId.value;
      pendingSummonerId.value = 0;

      if (summonerId) {
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

      const gn = summoner.value.gameName || summoner.value.displayName || name;
      const tl = summoner.value.tagLine;
      const historyKey = tl ? `${gn}#${tl}` : gn;
      saveToHistory(historyKey);

      if (summoner.value.puuid) {
        await loadMatchHistoryList();
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

      if (allMatchesSearch.value.length === 0) {
        const raw = await fetchMatchHistorySmart(
          summoner.value.puuid,
          0,
          INITIAL_BATCH - 1,
        );
        allMatchesSearch.value = raw;
        loadedGameIndex.value = Math.max(INITIAL_BATCH, raw.length);
      } else if (
        currentPageNum.value >= 2 &&
        end + matchesPerPage * PREFETCH_PAGES >= allFilteredMatches.value.length
      ) {
        schedulePrefetchMatches();
      }

      matches.value = allFilteredMatches.value.slice(beg, end);
      hasMore.value = allFilteredMatches.value.length > end;

      if (uploadEnabled.value && matches.value.length > 0) {
        const newIds = matches.value
          .map((m: MatchDisplay) => m.gameId)
          .filter((id: number) => !uploadedGameIds.value.has(id));
        if (newIds.length > 0) {
          newIds.forEach((id: number) => uploadedGameIds.value.add(id));
          batchUploadMatches(newIds)
            .then(
              (result: {
                successCount: number;
                failedCount: number;
                error: string | null;
              }) => {
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
              showToast(
                `上传异常: ${e instanceof Error ? e.message : String(e)}`,
                "error",
              );
            });
        }
      }
    } catch (e) {
      if (allMatchesSearch.value.length === 0) {
        matches.value = [];
        resetSelection();
      }
      showToast(`战绩列表获取失败: ${String(e)}`, "error");
    }
  }

  async function loadMoreMatches() {
    if (!summoner.value || loadingMore.value) return;
    loadingMore.value = true;
    const gen = searchGeneration;
    const puuid = summoner.value.puuid;
    try {
      const fetchBeg = loadedGameIndex.value;
      const fetchEnd = fetchBeg + LOAD_MORE_COUNT - 1;

      let raw = await fetchMatchHistory(puuid, fetchBeg, fetchEnd);
      if (gen !== searchGeneration) return;

      let existingIds = new Set(allMatchesSearch.value.map((m) => m.gameId));
      let newGames = raw.filter((m) => !existingIds.has(m.gameId));

      if (newGames.length === 0 && raw.length > 0) {
        try {
          raw = await fetchMatchHistorySgp(puuid, fetchBeg, fetchEnd);
          if (gen !== searchGeneration) return;
          existingIds = new Set(allMatchesSearch.value.map((m) => m.gameId));
          newGames = raw.filter((m) => !existingIds.has(m.gameId));
        } catch {
          /* keep empty */
        }
      }

      if (newGames.length > 0) {
        allMatchesSearch.value.push(...newGames);
        if (allMatchesSearch.value.length > 500) {
          allMatchesSearch.value = allMatchesSearch.value.slice(0, 500);
        }
        if (matches.value.length < matchesPerPage) {
          const beg = (currentPageNum.value - 1) * matchesPerPage;
          const end = beg + matchesPerPage;
          matches.value = allFilteredMatches.value.slice(beg, end);
        }
      }

      loadedGameIndex.value = fetchEnd;
      const currentEnd = currentPageNum.value * matchesPerPage;
      hasMore.value = allFilteredMatches.value.length > currentEnd;
    } catch {
      /* ignore */
    } finally {
      loadingMore.value = false;
    }
  }

  async function handlePrevPage() {
    if (searching.value || pageSwitching.value) return;
    if (currentPageNum.value > 1) {
      pageSwitching.value = true;
      try {
        currentPageNum.value--;
        await loadMatchHistoryList();
        if (matches.value.length > 0) {
          await selectMatch(matches.value[0].gameId);
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

      while (allFilteredMatches.value.length < requiredCount) {
        const prevCount = allMatchesSearch.value.length;
        await loadMoreMatches();
        if (allMatchesSearch.value.length === prevCount) {
          break;
        }
      }

      if (allFilteredMatches.value.length <= (targetPage - 1) * matchesPerPage) {
        hasMore.value = false;
        return;
      }

      currentPageNum.value = targetPage;
      await loadMatchHistoryList();
      if (matches.value.length > 0) {
        await selectMatch(matches.value[0].gameId);
      }
    } finally {
      pageSwitching.value = false;
    }
  }

  function selectQueue(id: number) {
    selectedQueue.value = id;
    currentPageNum.value = 1;
    void loadMatchHistoryList();
    if (matches.value.length > 0) {
      void selectMatch(matches.value[0].gameId);
    }
  }

  function selectHistoryItem(name: string) {
    searchName.value = name;
    showHistory.value = false;
    void doSearch();
  }

  return {
    searchName,
    searching,
    error,
    summoner,
    matches,
    selectedQueue,
    uploadEnabled,
    uploadedGameIds,
    currentPageNum,
    hasMore,
    allMatchesSearch,
    allFilteredMatches,
    currentRiotId,
    showHistory,
    filteredHistory,
    loadSearchHistory,
    removeFromHistory,
    hideHistoryDelayed,
    doSearch,
    waitForSearchIdle,
    loadMatchHistoryList,
    handlePrevPage,
    handleNextPage,
    selectQueue,
    selectHistoryItem,
    searchPlayerBySummonerId,
  };
}
