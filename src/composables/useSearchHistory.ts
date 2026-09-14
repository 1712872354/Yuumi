import { ref, computed, type Ref } from "vue";

const STORAGE_KEY = "yuumi_search_history";
const MAX_ITEMS = 20;

/** Search 页本地搜索历史（localStorage） */
export function useSearchHistory(searchName: Ref<string>, summonerName: Ref<string>) {
  const searchHistory = ref<string[]>([]);
  const showHistory = ref(false);

  function loadSearchHistory() {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) searchHistory.value = JSON.parse(saved);
    } catch {
      /* ignore */
    }
  }

  function persist() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(searchHistory.value));
  }

  function saveToHistory(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    searchHistory.value = [
      trimmed,
      ...searchHistory.value.filter((h) => h !== trimmed),
    ].slice(0, MAX_ITEMS);
    persist();
  }

  function removeFromHistory(name: string) {
    searchHistory.value = searchHistory.value.filter((h) => h !== name);
    persist();
  }

  const filteredHistory = computed(() => {
    const q = searchName.value.trim().toLowerCase();
    if (!q) return searchHistory.value;
    // 聚焦时输入框等于当前 Riot ID → 展示全部历史
    if (summonerName.value && q === summonerName.value.toLowerCase()) {
      return searchHistory.value;
    }
    return searchHistory.value.filter((h) => h.toLowerCase().includes(q));
  });

  function hideHistoryDelayed() {
    setTimeout(() => {
      showHistory.value = false;
    }, 200);
  }

  return {
    searchHistory,
    showHistory,
    filteredHistory,
    loadSearchHistory,
    saveToHistory,
    removeFromHistory,
    hideHistoryDelayed,
  };
}
