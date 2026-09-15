<script setup lang="ts">
import { inject } from "vue";
import { useLoot } from "../../composables/useLoot";
import LcuImage from "../LcuImage.vue";
import { NInputNumber } from "naive-ui";

type LootApi = ReturnType<typeof useLoot>;
const loot = inject<LootApi>("loot")!;
const {
  selectedLoot,
  openQuantity,
  maxOpenQuantity,
  keyDisplayName,
  currentKeyCount,
  getLootDisplayName,
  isKeyFragmentLoot,
  closeLootModal,
} = loot;

const props = defineProps<{
  onBatchOpen: () => void;
}>();
</script>

<template>
  <!-- 战利品开启数量选择弹窗 -->
  <Transition name="fade">
    <div
      v-if="selectedLoot"
      class="loot-modal-overlay"
      @click.self="closeLootModal"
    >
      <div class="loot-modal-card">
        <div class="loot-modal-header">
          <h3>
            {{ isKeyFragmentLoot(selectedLoot) ? $t("tools.lootOpener.batchForge") : $t("tools.lootOpener.batchOpen") }} - {{ getLootDisplayName(selectedLoot) }}
          </h3>
          <button class="modal-close-btn" @click="closeLootModal">&#x2715;</button>
        </div>
        <div class="loot-modal-body">
          <div class="loot-modal-preview">
            <div class="loot-modal-icon-container">
              <LcuImage :src="selectedLoot.tilePath ?? undefined" class="loot-modal-icon" />
            </div>
            <div class="loot-modal-details">
              <span class="loot-modal-name">{{ getLootDisplayName(selectedLoot) }}</span>
              <span class="loot-modal-owned">{{ $t("tools.lootOpener.owned") }}: x{{ selectedLoot.count }}</span>
            </div>
          </div>
          <div v-if="selectedLoot.needKey && selectedLoot.keyLootId" class="loot-info-row">
            <span class="loot-info-label">{{ $t("tools.lootOpener.keyRequired") }}</span>
            <span class="loot-info-value loot-key-count">
              {{ keyDisplayName }} x{{ openQuantity }} ({{ $t("tools.lootOpener.owned") }}: {{ currentKeyCount }})
            </span>
          </div>
          <div class="loot-quantity-row">
            <span class="loot-info-label">
              {{ isKeyFragmentLoot(selectedLoot) ? '合成次数' : $t("tools.lootOpener.quantity") }}
            </span>
            <n-input-number
              v-model:value="openQuantity"
              :min="1"
              :max="maxOpenQuantity"
              style="width: 120px"
              size="small"
            />
          </div>
          <div v-if="maxOpenQuantity <= 0" class="loot-insufficient">
            {{ isKeyFragmentLoot(selectedLoot) ? $t("tools.lootOpener.insufficientFragments") : (selectedLoot.needKey ? $t("tools.lootOpener.insufficientKeys") : '') }}
          </div>
              <div class="loot-modal-actions">
                <button class="action-btn" @click="closeLootModal">
                  {{ $t("tools.cancel") }}
                </button>
                <button
                  class="action-btn"
                  :disabled="maxOpenQuantity <= 0"
                  @click="props.onBatchOpen()"
                >
                  {{ isKeyFragmentLoot(selectedLoot) ? $t("tools.lootOpener.startForge", { count: openQuantity }) : $t("tools.lootOpener.startOpen", { count: openQuantity }) }}
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


</style>
