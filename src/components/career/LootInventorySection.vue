<script setup lang="ts">
import { inject } from "vue";
import { useLoot } from "../../composables/useLoot";
import { useLcuStore } from "../../store/lcuStore";
import LcuImage from "../LcuImage.vue";
import { NSelect, NInputGroup, NInputNumber } from "naive-ui";

type LootApi = ReturnType<typeof useLoot>;
const loot = inject<LootApi>("loot")!;
const store = useLcuStore();
const {
  isInventoryLoading,
  inventoryError,
  blueEssenceCount,
  orangeEssenceCount,
  filterType,
  filterOwned,
  filterValueType,
  filterOperator,
  filterMaxValue,
  filteredInventory,
  selectedLootIds,
  canUpgrade,
  upgradeBtnText,
  canReroll,
  gainBlueEssence,
  gainOrangeEssence,
  toggleSelectItem,
  handleSelectAllFiltered,
  handleClearSelection,
  handleBatchDisenchant,
  handleBatchUpgrade,
  handleBatchReroll,
  isOpening,
} = loot;

const props = defineProps<{
  onRefresh: () => void;
}>();
</script>

<template>
    <!-- 区块二：碎片库存管理 -->
    <div class="loot-section-card">
      <!-- 头部操作栏 -->
      <div class="loot-section-header">
        <div class="header-left">
          <span class="section-title">&#x1F48E; {{ $t("tools.lootManager.title") }}</span>
          <button
            class="action-btn"
            :disabled="!store.isConnected || isInventoryLoading"
            @click="props.onRefresh()"
          >
            {{ isInventoryLoading ? '正在刷新...' : $t("tools.lootManager.refreshBtn") }}
          </button>
        </div>
        <div class="header-right essence-header-balance">
          <span class="blue-essence-text">&#x1F535; {{ blueEssenceCount }}</span>
          <span class="orange-essence-text">&#x1F536; {{ orangeEssenceCount }}</span>
        </div>
      </div>

      <!-- 紧凑精美的水平筛选栏 -->
      <div class="loot-filter-bar-horizontal">
        <!-- 碎片类型 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">{{ $t("tools.lootManager.filterType") }}</span>
          <n-select
            v-model:value="filterType"
            :options="[
              { label: $t('tools.lootManager.filterAll'), value: 'ALL' },
              { label: '英雄', value: 'CHAMPION' },
              { label: '皮肤', value: 'SKIN' },
              { label: '表情', value: 'EMOTE' },
              { label: '守卫', value: 'WARDSKIN' },
              { label: '图标', value: 'SUMMONERICON' },
              { label: '永恒星碑', value: 'ETERNAL' },
              { label: '材料/宝箱', value: 'MATERIAL' },
            ]"
            size="small"
            style="width: 120px"
          />
        </div>

        <!-- 拥有状态 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">{{ $t("tools.lootManager.filterOwned") }}</span>
          <n-select
            v-model:value="filterOwned"
            :options="[
              { label: $t('tools.lootManager.filterOwnedAll'), value: 'ALL' },
              { label: $t('tools.lootManager.filterOwnedYes'), value: 'OWNED' },
              { label: $t('tools.lootManager.filterOwnedNo'), value: 'NOT_OWNED' },
            ]"
            size="small"
            style="width: 110px"
          />
        </div>

        <!-- 价值基准 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">{{ $t("tools.lootManager.filterValueType") }}</span>
          <n-select
            v-model:value="filterValueType"
            :options="[
              { label: $t('tools.lootManager.filterValueTypeDisenchant'), value: 'disenchantValue' },
              { label: $t('tools.lootManager.filterValueTypeStore'), value: 'value' },
            ]"
            size="small"
            style="width: 120px"
          />
        </div>

        <!-- 价值范围过滤 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">价值</span>
          <n-input-group>
            <n-select
              v-model:value="filterOperator"
              :options="[
                { label: '小于等于 (<=)', value: '<=' },
                { label: '大于等于 (>=)', value: '>=' },
                { label: '等于 (=)', value: '=' }
              ]"
              size="small"
              style="width: 115px"
            />
            <n-input-number
              v-model:value="filterMaxValue"
              :min="0"
              placeholder="不限"
              size="small"
              clearable
              style="width: 125px"
            />
          </n-input-group>
        </div>
      </div>

      <!-- 内容主体 -->
      <div class="loot-section-content">
        <!-- 加载态 -->
        <div v-if="isInventoryLoading" class="loot-loading-inline">
          <div class="loading-spinner"></div>
          <span>{{ $t("tools.lootManager.loading") }}</span>
        </div>

        <!-- 错误态 -->
        <div v-else-if="inventoryError" class="loot-error-inline">{{ inventoryError }}</div>

        <!-- 空态 -->
        <div v-else-if="filteredInventory.length === 0" class="loot-empty-inline">
          {{ $t("tools.lootManager.empty") }}
        </div>

        <!-- 碎片卡片网格 -->
        <div v-else class="loot-grid loot-inventory-grid">
          <div
            v-for="item in filteredInventory"
            :key="item.lootId"
            :class="['loot-card-item', { selected: selectedLootIds.includes(item.lootId) }]"
            @click="toggleSelectItem(item.lootId)"
            style="position: relative;"
          >
            <!-- 选中状态角标 -->
            <div v-if="selectedLootIds.includes(item.lootId)" class="selected-checkmark-badge">
              &#x2713;
            </div>
            <div class="loot-card-icon-container">
              <LcuImage :src="item.tilePath ?? undefined" class="loot-card-icon" />
            </div>
            <div class="loot-card-info">
              <div class="loot-card-header">
                <span class="loot-card-name" :title="item.itemDesc">{{ item.itemDesc }}</span>
                <span class="loot-card-count">x{{ item.count }}</span>
              </div>
              <div class="loot-card-footer loot-manager-card-footer">
                <span :class="item.itemStatus === 'OWNED' ? 'loot-badge-owned' : 'loot-badge-not-owned'">
                  {{ item.itemStatus === 'OWNED' ? $t("tools.lootManager.ownedBadge") : $t("tools.lootManager.notOwnedBadge") }}
                </span>
                <span v-if="item.displayCategories.toUpperCase() === 'SKIN' && item.itemStatus !== 'OWNED' && item.parentItemStatus !== 'OWNED'" class="loot-badge-no-parent">
                  {{ $t("tools.lootManager.noChampionBadge") }}
                </span>
                <span class="essence-badge" :class="item.displayCategories === 'CHAMPION' ? 'blue-essence-text' : 'orange-essence-text'">
                  {{ item.displayCategories === 'CHAMPION' ? '&#x1F535;' : '&#x1F536;' }} {{ item.disenchantValue }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 底部浮动控制栏 -->
      <div v-if="filteredInventory.length > 0" class="loot-batch-toolbar">
        <div class="toolbar-left">
          <button class="action-btn" @click="handleSelectAllFiltered">
            {{ $t("tools.lootManager.selectAll") }}
          </button>
          <button class="action-btn" @click="handleClearSelection">
            {{ $t("tools.lootManager.clearAll") }}
          </button>
          <span class="selected-count">
            {{ $t("tools.lootManager.selectedInfo", { count: selectedLootIds.length }) }}
          </span>
        </div>
        <div class="toolbar-right">
          <span v-if="selectedLootIds.length > 0" class="essence-preview">
            {{ $t("tools.lootManager.estimateEssence") }}
            <span class="blue-essence-text">&#x1F535; {{ gainBlueEssence }}</span>
            <span class="orange-essence-text">&#x1F536; {{ gainOrangeEssence }}</span>
          </span>
          <button
            class="action-btn"
            :disabled="selectedLootIds.length === 0 || isOpening || !store.isConnected"
            @click="handleBatchDisenchant"
          >
            {{ $t("tools.lootManager.disenchantBtn") }}
          </button>
          <button
            class="action-btn"
            :disabled="!canReroll || isOpening || !store.isConnected"
            @click="handleBatchReroll"
          >
            {{ $t("tools.lootManager.rerollBtn") }}
          </button>
          <button
            class="action-btn"
            :disabled="!canUpgrade || isOpening || !store.isConnected"
            @click="handleBatchUpgrade"
          >
            {{ upgradeBtnText }}
          </button>
        </div>
      </div>
    </div>

</template>

<style scoped>
/* ═════════ 战利品区块卡片 & 碎片库存管理 ═════════ */

.loot-section-card {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  box-shadow: var(--shadow-sm);
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
}

.loot-section-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md), 0 4px 20px rgba(0, 0, 0, 0.02);
}

.loot-section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 14px;
}

.essence-header-balance {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 0.85rem;
  font-weight: 700;
  background: var(--hover-bg);
  padding: 4px 12px;
  border-radius: 20px;
  border: 1px solid var(--border-color);
}

.loot-section-header .header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.section-title {
  font-size: 1rem;
  font-weight: 800;
  color: var(--text-color);
}

.loot-section-content {
  min-height: 80px;
  display: flex;
  flex-direction: column;
}

/* 水平筛选栏 */
.loot-filter-bar-horizontal {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  align-items: center;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 10px 16px;
}

.horizontal-filter-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-label-inline {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  white-space: nowrap;
}

/* 碎片卡片选择态 */
.loot-card-item.selected {
  border-color: var(--primary-color) !important;
  box-shadow: 0 8px 24px var(--primary-color-alpha-30), 0 0 0 2px var(--primary-color) !important;
  background: var(--primary-color-alpha-15) !important;
  transform: translateY(-4px) scale(1.02) !important;
}

/* 选中标记角标 */
.selected-checkmark-badge {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  background: var(--primary-color);
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 900;
  box-shadow: 0 2px 8px var(--primary-color-alpha-40);
  border: 1.5px solid #ffffff;
  z-index: 10;
  animation: popIn 0.25s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes popIn {
  from {
    transform: scale(0);
  }
  to {
    transform: scale(1);
  }
}

/* 精品率/拥有状态标签 */
.loot-badge-owned {
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.25);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

.loot-badge-not-owned {
  background: rgba(107, 114, 128, 0.1);
  color: var(--text-muted);
  border: 1px solid rgba(107, 114, 128, 0.2);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

.loot-badge-no-parent {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.65rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

/* 精粹数值 */
.essence-badge {
  font-size: 0.72rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  white-space: nowrap;
  flex-shrink: 0;
}
.blue-essence-text { color: #2563eb; }
.orange-essence-text { color: #d97706; }

/* 内置加载/错误/空态 */
.loot-loading-inline,
.loot-error-inline,
.loot-empty-inline {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: var(--text-dimmed);
  font-size: 0.88rem;
  gap: 12px;
  width: 100%;
  box-sizing: border-box;
}

.loot-error-inline {
  color: var(--loss-color);
}

.action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  line-height: 1.4;
  font-family: inherit;
}

.action-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
}

.modal-close-btn {
  background: none;

/* 水平筛选栏 */
.loot-filter-bar-horizontal {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  align-items: center;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 10px 16px;
}

.horizontal-filter-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-label-inline {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  white-space: nowrap;
}

/* 碎片卡片选择态 */
.loot-card-item.selected {
  border-color: var(--primary-color) !important;
  box-shadow: 0 8px 24px var(--primary-color-alpha-30), 0 0 0 2px var(--primary-color) !important;
  background: var(--primary-color-alpha-15) !important;
  transform: translateY(-4px) scale(1.02) !important;
}

/* 选中标记角标 */
.selected-checkmark-badge {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  background: var(--primary-color);
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 900;
  box-shadow: 0 2px 8px var(--primary-color-alpha-40);
  border: 1.5px solid #ffffff;
  z-index: 10;
  animation: popIn 0.25s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes popIn {
  from {
    transform: scale(0);
  }
  to {
    transform: scale(1);
  }
}

/* 精品率/拥有状态标签 */
.loot-badge-owned {
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.25);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

.loot-badge-not-owned {
  background: rgba(107, 114, 128, 0.1);
  color: var(--text-muted);
  border: 1px solid rgba(107, 114, 128, 0.2);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

.loot-badge-no-parent {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.65rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

/* 精粹数值 */
.essence-badge {
  font-size: 0.72rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  white-space: nowrap;
  flex-shrink: 0;
}
.blue-essence-text { color: #2563eb; }
.orange-essence-text { color: #d97706; }

/* 内置加载/错误/空态 */
.loot-loading-inline,
.loot-error-inline,
.loot-empty-inline {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: var(--text-dimmed);
  font-size: 0.88rem;
  gap: 12px;
  width: 100%;
  box-sizing: border-box;
}

.loot-error-inline {
  color: var(--loss-color);
}

/* 底部浮动控制栏 */
.loot-batch-toolbar {
  position: sticky;
  bottom: 0;
  background: var(--settings-card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 12px 18px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  box-shadow: var(--shadow-lg);
  z-index: 10;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  margin-top: 16px;
}

.loot-batch-toolbar .toolbar-left,
.loot-batch-toolbar .toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.selected-count {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  margin-left: 6px;
}

.essence-preview {
  font-size: 0.82rem;
  font-weight: 700;
  display: flex;
  gap: 8px;
  align-items: center;
  margin-right: 6px;
}

/* 碎片网格负边距修正 */
.loot-inventory-grid {
  padding-bottom: 4px;
}
/* 底部浮动控制栏 */
.loot-batch-toolbar {
  position: sticky;
  bottom: 0;
  background: var(--settings-card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 12px 18px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  box-shadow: var(--shadow-lg);
  z-index: 10;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  margin-top: 16px;
}

.loot-batch-toolbar .toolbar-left,
.loot-batch-toolbar .toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.selected-count {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  margin-left: 6px;
}

.essence-preview {
  font-size: 0.82rem;
  font-weight: 700;
  display: flex;
  gap: 8px;
  align-items: center;
  margin-right: 6px;
}

/* 碎片网格负边距修正 */
.loot-inventory-grid {
  padding-bottom: 4px;
}

}
</style>
