<script setup lang="ts">
import { inject } from "vue";
import { useLoot } from "../../composables/useLoot";
import { NProgress, NButton } from "naive-ui";

type LootApi = ReturnType<typeof useLoot>;
const loot = inject<LootApi>("loot")!;
const {
  showOpenPanel,
  progressPanelTitle,
  openPercentage,
  openProgress,
  openTotal,
  openResults,
  isOpening,
  closeOpenPanel,
} = loot;
</script>

<template>
  <!-- 批量开启进度面板 -->
  <Transition name="slide-up">
    <div v-if="showOpenPanel" class="loot-progress-overlay">
      <div class="loot-progress-card">
        <div class="loot-progress-header">
          <h3>{{ progressPanelTitle }}</h3>
          <button
            class="modal-close-btn"
            :disabled="isOpening"
            @click="closeOpenPanel"
          >&#x2715;</button>
        </div>
        <div class="loot-progress-body">
          <n-progress
            type="line"
            :percentage="openPercentage"
            :indicator-placement="'inside'"
            :border-radius="4"
            :height="20"
            status="success"
          />
          <div class="loot-progress-info">
            {{ openProgress }} / {{ openTotal }}
          </div>
          <div class="loot-results-list">
            <div
              v-for="(result, idx) in openResults"
              :key="idx"
              :class="['loot-result-item', result.success ? 'success' : 'error']"
            >
              <span class="loot-result-icon">{{ result.success ? '&#x1F389;' : '&#x274C;' }}</span>
              <span class="loot-result-text">
                [{{ result.current }}/{{ result.total }}]
                {{ result.success ? result.rewardName : result.errorMsg }}
              </span>
            </div>
          </div>
          <div v-if="!isOpening" class="loot-progress-actions" style="margin-top: 14px;">
            <n-button type="primary" size="medium" block @click="closeOpenPanel">
              确定
            </n-button>
          </div>
        </div>
      </div>
    </div>
  </Transition>

</template>

<style scoped>
/* 批量开启进度面板 */
.loot-progress-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.loot-progress-card {
  width: 420px;
  max-height: 80vh;
  background: var(--settings-card-bg, rgba(255, 255, 255, 0.95));
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.loot-progress-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: var(--hover-bg);
}

.loot-progress-header h3 {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
}

.loot-progress-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
}

.loot-progress-info {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  font-weight: 600;
  text-align: center;
}

.loot-results-list {
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.loot-result-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 0.78rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  animation: lootResultIn 0.3s ease-out;
}

.loot-result-item.success {
  border-color: var(--win-border);
  background: var(--win-bg);
}

.loot-result-item.error {
  border-color: var(--loss-border);
  background: var(--loss-bg);
}

.loot-result-icon {
  font-size: 0.9rem;
  flex-shrink: 0;
}

.loot-result-text {
  color: var(--text-color);
  word-break: break-all;
}

@keyframes lootResultIn {
  from {
    opacity: 0;
    transform: translateX(-8px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}


</style>
