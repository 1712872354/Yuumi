<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getPipelineStats } from "../../api/lcu";
import type { PipelineStatsSnapshot } from "../../types/events";
import { useToast } from "../../composables/useToast";

const { t } = useI18n();
const { showToast } = useToast();
const stats = ref<PipelineStatsSnapshot | null>(null);
const loading = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

async function refresh() {
  loading.value = true;
  try {
    stats.value = await getPipelineStats();
  } catch (e) {
    console.error("[Tools] 获取管道统计失败:", e);
    showToast(t("tools_extra.pipelineStats.failed"), "error");
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void refresh();
  timer = setInterval(() => {
    void refresh();
  }, 5000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});

const rows = () => {
  const s = stats.value;
  if (!s) return [];
  return [
    { key: "wsEventsEmitted", value: s.wsEventsEmitted },
    { key: "champSelectThrottled", value: s.champSelectThrottled },
    { key: "bpChannelDropped", value: s.bpChannelDropped },
    { key: "matchChannelDropped", value: s.matchChannelDropped },
    {
      key: "matchDetailCache",
      value: `${s.matchDetailCacheHits} / ${s.matchDetailCacheMisses}`,
    },
    { key: "uploadEnqueued", value: s.uploadEnqueued },
    { key: "uploadSuccess", value: s.uploadSuccess },
    { key: "uploadFailed", value: s.uploadFailed },
  ];
};
</script>

<template>
  <div class="card-item border-bottom pipeline-stats-card">
    <div class="card-left">
      <div class="icon-container">
        <svg
          class="header-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M3 3v18h18"></path>
          <path d="M7 16l4-8 4 4 4-6"></path>
        </svg>
      </div>
      <div class="title-container">
        <h3 class="card-title">{{ t("tools_extra.pipelineStats.title") }}</h3>
        <span class="card-desc">{{ t("tools_extra.pipelineStats.desc") }}</span>
      </div>
    </div>
    <div class="card-right stats-grid">
      <div v-if="!stats && loading" class="stats-loading">…</div>
      <template v-else-if="stats">
        <div v-for="row in rows()" :key="row.key" class="stat-row">
          <span class="stat-label">
            {{ t(`tools_extra.pipelineStats.${row.key}`) }}
          </span>
          <span class="stat-value">{{ row.value }}</span>
        </div>
      </template>
      <button class="action-btn refresh-btn" :disabled="loading" @click="refresh">
        {{ t("tools_extra.pipelineStats.refresh") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.pipeline-stats-card {
  align-items: flex-start;
}

.card-item {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  border-radius: 12px;
  margin-bottom: 8px;
  box-shadow: var(--shadow-sm);
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-item.border-bottom {
  border-radius: 12px;
}

.card-left {
  display: flex;
  align-items: flex-start;
  flex: 1;
  gap: 14px;
}

.icon-container {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: var(--settings-icon-bg);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.header-icon {
  width: 18px;
  height: 18px;
}

.title-container {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.card-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-primary);
}

.card-desc {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.stats-grid {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 220px;
  text-align: right;
}

.stat-row {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  font-size: 0.85rem;
  line-height: 1.4;
}

.stat-label {
  color: var(--text-muted);
}

.stat-value {
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.stats-loading {
  color: var(--text-muted);
  padding: 4px 0;
}

.action-btn {
  border: 1px solid var(--settings-card-border);
  background: var(--settings-card-bg-hover);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 0.85rem;
  cursor: pointer;
}

.action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.refresh-btn {
  margin-top: 6px;
  align-self: flex-end;
}
</style>
