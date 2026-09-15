<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { NInput, NButton, NSelect, NCheckbox } from "naive-ui";
import { QUEUE_FILTER_OPTIONS } from "../../utils/queueMeta";

defineProps<{
  searchName: string;
  searching: boolean;
  selectedQueue: number;
  uploadEnabled: boolean;
  showUploadCheckbox: boolean;
  showHistory: boolean;
  filteredHistory: string[];
}>();

const emit = defineEmits<{
  "update:searchName": [value: string];
  "update:selectedQueue": [value: number];
  "update:uploadEnabled": [value: boolean];
  search: [];
  selectHistory: [name: string];
  removeHistory: [name: string];
  hideHistory: [];
  showHistoryPanel: [];
  goCareer: [];
}>();

const { t } = useI18n();
const QUEUE_OPTIONS = QUEUE_FILTER_OPTIONS;
void t;
void emit;
</script>

<template>
  <div class="search-bar">
    <div class="search-input-wrapper">
      <n-input
        :value="searchName"
        :placeholder="t('search.searchPlaceholder')"
        :disabled="searching"
        clearable
        style="width: 100%"
        size="small"
        @update:value="(v: string) => emit('update:searchName', v)"
        @keyup.enter="emit('search')"
        @focus="emit('showHistoryPanel')"
        @click="emit('showHistoryPanel')"
        @blur="emit('hideHistory')"
      >
        <template #suffix>
          <n-button
            quaternary
            circle
            size="tiny"
            :disabled="searching || !searchName.trim()"
            @click="emit('search')"
          >
            <template #icon>
              <svg
                class="search-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <circle cx="11" cy="11" r="8" />
                <line x1="21" y1="21" x2="16.65" y2="16.65" />
              </svg>
            </template>
          </n-button>
        </template>
      </n-input>
      <div v-if="showHistory && filteredHistory.length > 0" class="history-dropdown">
        <div class="history-header">
          <span class="history-title">🕐 {{ $t("search.history") }}</span>
        </div>
        <div class="history-tags-container">
          <div
            v-for="item in filteredHistory"
            :key="item"
            class="history-tag"
            @mousedown.prevent="emit('selectHistory', item)"
          >
            <span class="history-text" :title="item">{{ item }}</span>
            <span
              class="history-delete"
              :title="$t('tools.cancel')"
              @mousedown.prevent.stop="emit('removeHistory', item)"
              >✕</span
            >
          </div>
        </div>
      </div>
    </div>

    <n-button size="small" @click="emit('goCareer')">{{
      $t("nav.career")
    }}</n-button>

    <n-select
      :value="selectedQueue"
      :options="
        QUEUE_OPTIONS.map((q) => ({
          label:
            q.id === null || q.id === -1
              ? $t('career.all')
              : $t('gameModes.' + q.id),
          value: q.id === null ? -1 : q.id,
        }))
      "
      style="width: 130px"
      size="small"
      @update:value="(v: number) => emit('update:selectedQueue', v)"
    />

    <n-checkbox
      v-if="showUploadCheckbox"
      :checked="uploadEnabled"
      @update:checked="(v: boolean) => emit('update:uploadEnabled', v)"
    >
      Upload matches
    </n-checkbox>
  </div>
</template>

<style scoped>
.search-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.search-input-wrapper {
  position: relative;
  flex: 1;
  min-width: 0;
}
.search-icon {
  width: 14px;
  height: 14px;
}
.history-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  z-index: 20;
  background: var(--card-bg, #fff);
  border: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
  border-radius: 8px;
  box-shadow: var(--shadow-md, 0 8px 24px rgba(0, 0, 0, 0.12));
  padding: 8px;
}
.history-header {
  margin-bottom: 6px;
}
.history-title {
  font-size: 12px;
  color: var(--text-muted, #6b7280);
}
.history-tags-container {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.history-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.05);
  cursor: pointer;
  font-size: 12px;
}
.history-delete {
  color: var(--text-dimmed, #9ca3af);
  font-size: 10px;
}
</style>
