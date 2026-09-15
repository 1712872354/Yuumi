<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { NSpin, NEmpty, NButton } from "naive-ui";
import {
  useTftMetaDecks,
  getDeckDisplayName,
  getChampionDisplayName,
  getTraitDisplayName,
  getItemDisplayName,
  getItemIconUrl,
  type TftMetaDeck,
  type TftMetaUnit,
} from "../../composables/useTftMetaDecks";
import { useToast } from "../../composables/useToast";
import {
  formatTftVersionLabel,
  formatPercent,
  formatPlacement,
  rankDecksWithTiers,
} from "../../utils/tftMetaDisplay";
import { computeDisplayBoard } from "../../utils/tftMetaBoard";
import LcuImage from "../LcuImage.vue";
import TftDeckCard from "./TftDeckCard.vue";

const { t } = useI18n();
const { showToast } = useToast();
const { loading, error, decks, metadata, loadDecks, refresh } = useTftMetaDecks();
const selectedDeck = ref<TftMetaDeck | null>(null);
const showDetailModal = ref(false);
const isFallbackBoard = ref(false);

const sortedDecks = computed(() => rankDecksWithTiers(decks.value));

const tftVersion = computed(() =>
  formatTftVersionLabel(decks.value, metadata.value?.gameStatDateTime),
);

async function copyTeamCode(code: string | undefined) {
  if (!code) return;
  try {
    await navigator.clipboard.writeText(code);
    showToast("阵容代码已复制到剪贴板，可进入游戏粘贴导入", "success");
  } catch (e) {
    console.error("复制代码失败:", e);
    showToast("复制阵容代码失败", "error");
  }
}

const boardUnitMap = computed(() => {
  const map = new Map<string, TftMetaUnit>();
  for (const u of displayUnits.value) {
    if (u.cell && typeof u.cell.x === "number" && typeof u.cell.y === "number") {
      map.set(`${u.cell.x}-${u.cell.y}`, u);
    }
  }
  return map;
});

function getBoardUnitCached(x: number, y: number): TftMetaUnit | undefined {
  return boardUnitMap.value.get(`${x}-${y}`);
}

const displayBoard = computed(() => computeDisplayBoard(selectedDeck.value?.units));
const displayUnits = computed(() => displayBoard.value.units);

watch(
  () => displayBoard.value.isFallback,
  (v) => {
    isFallbackBoard.value = v;
  },
  { immediate: true },
);

function selectDeck(deck: TftMetaDeck) {
  selectedDeck.value = deck;
  showDetailModal.value = true;
}

const selectedDeckIndex = computed(() => {
  if (!selectedDeck.value || !sortedDecks.value.length) return -1;
  return sortedDecks.value.findIndex((item) => item.deck.id === selectedDeck.value?.id);
});

const hasPrevDeck = computed(() => selectedDeckIndex.value > 0);
const hasNextDeck = computed(
  () => selectedDeckIndex.value >= 0 && selectedDeckIndex.value < sortedDecks.value.length - 1,
);

function prevDeck() {
  if (hasPrevDeck.value) {
    selectedDeck.value = sortedDecks.value[selectedDeckIndex.value - 1].deck;
  }
}

function nextDeck() {
  if (hasNextDeck.value) {
    selectedDeck.value = sortedDecks.value[selectedDeckIndex.value + 1].deck;
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (!showDetailModal.value) return;
  if (e.key === "ArrowLeft") {
    prevDeck();
  } else if (e.key === "ArrowRight") {
    nextDeck();
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
  loadDecks();
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div class="meta-comps-tab">
    <div class="header-row">
      <div class="header-left">
        <span class="header-desc">{{ t("tftPage.recommendDesc") }}</span>
        <span v-if="tftVersion" class="version-badge">🌐 数据版本: {{ tftVersion }}</span>
      </div>
      <n-button size="tiny" quaternary :loading="loading" @click="refresh">
        刷新
      </n-button>
    </div>

    <div v-if="loading && decks.length === 0" class="loading-box">
      <n-spin size="medium" />
    </div>

    <div v-else-if="error && decks.length === 0" class="error-box">
      <p>{{ error }}</p>
      <n-button size="small" type="primary" @click="refresh">重试</n-button>
    </div>

    <div v-else-if="decks.length === 0" class="empty-box">
      <n-empty :description="t('tftPage.noMetaDecks')" />
    </div>

    <template v-else>
      <div class="comp-grid">
        <TftDeckCard
          v-for="item in sortedDecks"
          :key="item.deck.id"
          :deck="item.deck"
          :tier="item.tier"
          :selected="selectedDeck?.id === item.deck.id"
          @select="selectDeck"
        />
      </div>

      <!-- 阵容详情弹窗（保留原弹窗结构；后续可再拆） -->
      <n-modal
        v-model:show="showDetailModal"
        preset="card"
        class="deck-detail-modal"
        :title="selectedDeck ? getDeckDisplayName(selectedDeck) : ''"
        style="width: 920px"
        :bordered="false"
      >
        <template #header-extra>
          <div class="modal-nav">
            <n-button size="tiny" quaternary :disabled="!hasPrevDeck" @click="prevDeck">
              ←
            </n-button>
            <n-button size="tiny" quaternary :disabled="!hasNextDeck" @click="nextDeck">
              →
            </n-button>
          </div>
        </template>
        <div v-if="selectedDeck" class="deck-detail-body">
          <div class="detail-stats">
            <span v-if="selectedDeck.stat?.win_rate != null">
              胜率 {{ formatPercent(selectedDeck.stat.win_rate) }}
            </span>
            <span v-if="selectedDeck.stat?.top4_rate != null">
              Top4 {{ formatPercent(selectedDeck.stat.top4_rate) }}
            </span>
            <span v-if="selectedDeck.stat?.avg_placement != null">
              均名 {{ formatPlacement(selectedDeck.stat.avg_placement) }}
            </span>
            <span v-if="selectedDeck.teamCode" class="team-code-actions">
              <n-button size="tiny" @click="copyTeamCode(selectedDeck.teamCode)">
                复制阵容代码
              </n-button>
            </span>
          </div>

          <div v-if="isFallbackBoard" class="board-legend-tip">
            ⚠️ 源数据无站位，已按前排/后排智能摆放（仅供参考）
          </div>

          <div class="board-traits-layout">
            <div class="board-side">
              <div v-for="y in 4" :key="y" class="board-row">
                <div v-for="x in 8" :key="x" class="board-cell">
                  <div
                    v-if="getBoardUnitCached(x - 1, y - 1)"
                    class="board-unit-wrap"
                  >
                    <div class="board-unit-box">
                      <LcuImage
                        :src="`/lol-game-data/assets/v1/champion-icons/${getBoardUnitCached(x - 1, y - 1)!.characterId || 0}.png`"
                        class="board-unit-img"
                      />
                      <span class="board-unit-text">
                        {{
                          getChampionDisplayName(
                            getBoardUnitCached(x - 1, y - 1)!.characterId ||
                              getBoardUnitCached(x - 1, y - 1)!.key ||
                              "",
                          )
                        }}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <div class="board-right-side">
              <div v-if="selectedDeck.traits?.length" class="trait-list">
                <div
                  v-for="tr in selectedDeck.traits"
                  :key="tr.key"
                  class="trait-row"
                >
                  {{ getTraitDisplayName(tr.key) }}
                </div>
              </div>
              <div v-if="selectedDeck.units?.length" class="unit-item-list">
                <div
                  v-for="u in selectedDeck.units"
                  :key="u.key || u.characterId"
                  class="unit-line"
                >
                  <span class="unit-name">{{
                    getChampionDisplayName(u.characterId || u.key || "")
                  }}</span>
                  <span class="unit-items">
                    <LcuImage
                      v-for="(it, ii) in (u.items || []).slice(0, 3)"
                      :key="ii"
                      :src="getItemIconUrl(it)"
                      class="board-item-img"
                      :title="getItemDisplayName(String(it))"
                    />
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </n-modal>
    </template>
  </div>
</template>

<style scoped>
.meta-comps-tab {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  flex: 1;
}
.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.header-desc {
  color: var(--text-muted);
  font-size: 0.85rem;
}
.version-badge {
  font-size: 0.75rem;
  color: var(--text-dimmed);
  background: rgba(0, 0, 0, 0.15);
  padding: 2px 8px;
  border-radius: 999px;
}
.loading-box,
.empty-box,
.error-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 48px 16px;
  color: var(--text-muted);
}
.comp-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
}
.modal-nav {
  display: flex;
  gap: 4px;
}
.deck-detail-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.detail-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: center;
  font-size: 0.9rem;
  color: var(--text-color);
}
.board-legend-tip {
  font-size: 0.8rem;
  color: #f59e0b;
}
.board-traits-layout {
  display: flex;
  gap: 16px;
}
.board-side {
  flex: 1;
}
.board-row {
  display: flex;
  gap: 4px;
  margin-bottom: 4px;
}
.board-cell {
  width: 48px;
  height: 56px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
}
.board-unit-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}
.board-unit-img {
  width: 32px;
  height: 32px;
  border-radius: 6px;
}
.board-unit-text {
  font-size: 0.6rem;
  color: var(--text-muted);
  max-width: 44px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.board-right-side {
  width: 260px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.trait-list,
.unit-item-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.trait-row,
.unit-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.85rem;
  color: var(--text-color);
}
.unit-items {
  display: flex;
  gap: 4px;
}
.board-item-img {
  width: 22px;
  height: 22px;
  border-radius: 4px;
}
</style>
