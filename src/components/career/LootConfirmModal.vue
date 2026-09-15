<script setup lang="ts">
import { inject } from "vue";
import { useLoot } from "../../composables/useLoot";

type LootApi = ReturnType<typeof useLoot>;
const loot = inject<LootApi>("loot")!;
const { showConfirmModal, confirmModalConfig, executeConfirmedAction } = loot;
</script>

<template>
  <!-- 自定义确认操作弹窗 -->
  <Transition name="fade">
    <div
      v-if="showConfirmModal"
      class="loot-modal-overlay"
      @click.self="showConfirmModal = false"
    >
      <div class="loot-modal-card confirm-modal-card">
        <div class="loot-modal-header confirm-modal-header" :class="confirmModalConfig.type">
          <h3>&#x26A0;&#xFE0F; {{ confirmModalConfig.title }}</h3>
          <button class="modal-close-btn" @click="showConfirmModal = false">&#x2715;</button>
        </div>
        <div class="loot-modal-body confirm-modal-body">
          <p class="confirm-message">{{ confirmModalConfig.message }}</p>

          <!-- 额外详情信息 -->
          <div v-if="confirmModalConfig.details" class="confirm-details-box">
            <div
              v-for="(detail, index) in confirmModalConfig.details"
              :key="index"
              class="confirm-detail-row"
            >
              <span class="detail-label">{{ detail.label }}</span>
              <span class="detail-value" :class="detail.class">{{ detail.value }}</span>
            </div>
          </div>

          <div class="loot-modal-actions confirm-modal-actions">
            <button class="action-btn" @click="showConfirmModal = false">
              {{ confirmModalConfig.cancelText }}
            </button>
            <button
              class="action-btn"
              @click="executeConfirmedAction"
            >
              {{ confirmModalConfig.confirmText }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>

</template>

<style scoped>
/* 数量选择弹窗 */
.loot-modal-overlay {
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

.loot-modal-card {
  width: 380px;
  background: var(--settings-card-bg, rgba(255, 255, 255, 0.95));
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.loot-modal-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: var(--hover-bg);
}

.loot-modal-header h3 {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.loot-modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.loot-modal-preview {
  display: flex;
  align-items: center;
  gap: 16px;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  padding: 12px;
  border-radius: 10px;
}

.loot-modal-icon-container {
  width: 56px;
  height: 56px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.loot-modal-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.loot-modal-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.loot-modal-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
}

.loot-modal-owned {
  font-size: 0.78rem;
  color: var(--text-muted);
}

.loot-info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-info-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.loot-info-value {
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-color);
}

.loot-key-count {
  color: var(--warning-color, #e6a23c);
}

.loot-quantity-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 0;
  border-top: 1px dashed var(--border-color);
  border-bottom: 1px dashed var(--border-color);
}

.loot-insufficient {
  font-size: 0.78rem;
  color: var(--loss-color);
  font-weight: 600;
  text-align: center;
}

.loot-modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding-top: 6px;
}

/* 自定义确认弹窗特殊样式 */
.confirm-modal-card {
  width: 350px !important;
}

.confirm-message {
  font-size: 0.85rem;
  color: var(--text-color);
  line-height: 1.5;
  margin: 0 0 12px 0;
}

.confirm-details-box {
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}

.confirm-detail-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.78rem;
}

.confirm-detail-row .detail-label {
  color: var(--text-muted);
}

.confirm-detail-row .detail-value {
  font-weight: 700;
  color: var(--text-color);
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
  border: none;
  font-size: 1rem;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}
</style>
