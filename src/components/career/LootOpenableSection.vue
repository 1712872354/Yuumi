<script setup lang="ts">
import { inject } from "vue";
import { useLoot } from "../../composables/useLoot";
import { useLcuStore } from "../../store/lcuStore";
import LcuImage from "../LcuImage.vue";

type LootApi = ReturnType<typeof useLoot>;
const loot = inject<LootApi>("loot")!;
const store = useLcuStore();
const {
  sortedOpenableLoots,
  lootLoading,
  lootError,
  lootFetched,
  openableLoots,
  totalKeyCount,
  isOpening,
  openLootModal,
  getLootDisplayName,
  isKeyFragmentLoot,
} = loot;

const props = defineProps<{
  onRefresh: () => void;
  onSmartOpenAll: () => void;
}>();
</script>

<template>
  <!-- 区块一：可开启战利品（箱子/法球） -->
  <div class="loot-section-card">
    <!-- 头部操作栏 -->
    <div class="loot-section-header">
      <div class="header-left">
        <span class="section-title">&#x1F4E6; {{ $t("tools.lootManager.chestOpen") }}</span>
        <button
          class="action-btn"
          :disabled="!store.isConnected || lootLoading"
          @click="props.onRefresh()"
        >
          {{ lootLoading ? '正在刷新...' : $t("tools.lootOpener.refreshBtn") }}
        </button>
        <span v-if="openableLoots.length > 0" class="loot-key-summary">
          &#x1F511; x{{ totalKeyCount }}
        </span>
      </div>
      <button
        v-if="openableLoots.length > 0"
        class="action-btn"
        :disabled="!store.isConnected || isOpening || totalKeyCount <= 0"
        @click="props.onSmartOpenAll()"
      >
        {{ $t("tools.lootOpener.smartOpenAll") }}
      </button>
    </div>

      <!-- 内容主体 -->
      <div class="loot-section-content">
        <div v-if="lootLoading" class="loot-loading-inline">
          <div class="loading-spinner"></div>
          <span>{{ $t("tools.lootOpener.loading") }}</span>
        </div>
        <div v-else-if="lootError" class="loot-error-inline">{{ lootError }}</div>
        <div v-else-if="sortedOpenableLoots.length > 0" class="loot-grid">
          <div
            v-for="lootLoot in sortedOpenableLoots"
            :key="lootLoot.lootId"
            class="loot-card-item"
            @click="openLootModal(lootLoot)"
          >
            <div class="loot-card-icon-container">
              <LcuImage :src="lootLoot.tilePath ?? undefined" class="loot-card-icon" />
            </div>
            <div class="loot-card-info">
              <div class="loot-card-header">
                <span class="loot-card-name" :title="getLootDisplayName(lootLoot)">{{ getLootDisplayName(lootLoot) }}</span>
                <span class="loot-card-count">x{{ lootLoot.count }}</span>
              </div>
              <div class="loot-card-footer">
                <span v-if="isKeyFragmentLoot(lootLoot)" class="loot-no-key-badge" style="background: rgba(16, 185, 129, 0.15); color: #10b981;">
                  {{ $t("tools.lootOpener.forge3in1") }}
                </span>
                <span v-else-if="lootLoot.needKey" class="loot-key-badge">{{ $t("tools.lootOpener.needKey") }}</span>
                <span v-else class="loot-no-key-badge">{{ $t("tools.lootOpener.noKeyNeeded") }}</span>
                <span class="loot-open-btn">
                  {{ isKeyFragmentLoot(lootLoot) ? $t("tools.lootOpener.forgeBtn") : $t("tools.lootOpener.openBtn") }}
                </span>
              </div>
            </div>
          </div>
        </div>
        <div v-else class="loot-empty-inline">
          {{ lootFetched ? $t("tools.lootOpener.noLootFound") : $t("tools.lootOpener.clickRefresh") }}
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

}
</style>
