<script setup lang="ts">
import { onMounted, onUnmounted, watch, provide } from 'vue'
import { useLoot } from '../../composables/useLoot'
import { useLcuStore } from '../../store/lcuStore'
import LootOpenableSection from './LootOpenableSection.vue'
import LootInventorySection from './LootInventorySection.vue'
import LootConfirmModal from './LootConfirmModal.vue'
import LootOpenQuantityModal from './LootOpenQuantityModal.vue'
import LootProgressPanel from './LootProgressPanel.vue'

const props = defineProps<{
  refreshSummoner?: () => void
  active?: boolean
}>()

const store = useLcuStore()
const loot = useLoot()
provide('loot', loot)

// 惰性加载：仅当战利品 tab 首次激活时才拉取数据，避免应用启动即触发请求
let lootLoadedOnce = false

watch(
  () => props.active,
  (active) => {
    if (active && !lootLoadedOnce) {
      lootLoadedOnce = true
      if (store.isConnected) {
        loot.loadAllData()
      }
    }
  },
  { immediate: true },
)

function handleSmartOpenAll() {
  loot.handleSmartOpenAll(() => props.refreshSummoner?.())
}

function handleBatchOpenWithRefresh() {
  loot.handleBatchOpen(() => props.refreshSummoner?.())
}

onMounted(() => {
  loot.setRefreshCallback(() => props.refreshSummoner?.())
})

onUnmounted(() => {
  loot.cleanup()
})
</script>

<template>
  <div class="loot-tab-root">
    <div class="loot-tab-container">
      <LootOpenableSection
        :on-refresh="loot.loadLootData"
        :on-smart-open-all="handleSmartOpenAll"
      />
      <LootInventorySection :on-refresh="loot.loadLootInventory" />
    </div>

    <LootConfirmModal />
    <LootOpenQuantityModal :on-batch-open="handleBatchOpenWithRefresh" />
    <LootProgressPanel />
  </div>
</template>

<style scoped>
.loot-tab-root {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.loot-tab-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex: 1;
  overflow-y: auto;
}

/* Fade / slide transitions shared by modals */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.25s ease, opacity 0.25s ease;
}
.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(16px);
}
</style>
