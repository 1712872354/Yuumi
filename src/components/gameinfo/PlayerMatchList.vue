<script setup lang="ts">
import { NVirtualList } from "naive-ui";
import type { MatchDisplay } from "../../api/lcu";
import { getChampionIcon } from "../../types/gameInfo";
import LcuImage from "../LcuImage.vue";

defineProps<{
  matches: MatchDisplay[];
  isLoading: boolean;
  isMatchHidden: boolean;
  isHistoryUnreachable: boolean;
}>();

function copyGameId(e: MouseEvent, gameId: number) {
  e.stopPropagation();
  if (!gameId) return;
  navigator.clipboard?.writeText(String(gameId)).catch(() => {});
}
</script>

<template>
  <div class="matches">
    <div v-if="isLoading" class="empty">
      <span class="spinner"></span>
      <span>{{ $t("career.loading") }}</span>
    </div>
    <div v-else-if="isMatchHidden" class="empty">
      🔒 {{ $t("gameInfo.matchHidden", "战绩已隐藏") }}
    </div>
    <NVirtualList
      v-else-if="matches.length"
      class="match-list"
      key-field="gameId"
      :item-size="36"
      :items="matches"
    >
      <template #default="{ item: match }">
        <div
          class="mi"
          :class="{
            'mi-win': match.win === true,
            'mi-loss': match.win === false,
            'mi-remake': match.win === null || match.remake,
          }"
          :title="`${match.name || ''} · ${match.duration || ''}\n点击复制对局 ID`"
          @click="copyGameId($event, match.gameId)"
        >
          <LcuImage :src="getChampionIcon(match.championId)" class="mi-champ" />
          <div class="mi-mid">
            <span class="mi-mode">{{ match.name || "" }}</span>
            <span class="mi-time">
              {{ match.shortTime || match.time }}
              <span v-if="match.duration" class="mi-dur">{{ match.duration }}</span>
              <span v-if="match.remake" class="mi-remake-tag">重开</span>
            </span>
          </div>
          <div class="mi-right">
            <div class="mi-kda">
              <span class="k">{{ match.kills }}</span>
              <span class="s">/</span>
              <span class="d">{{ match.deaths }}</span>
              <span class="s">/</span>
              <span class="a">{{ match.assists }}</span>
            </div>
            <div v-if="match.cs" class="mi-cs">{{ match.cs }} CS</div>
          </div>
        </div>
      </template>
    </NVirtualList>
    <div v-else-if="isHistoryUnreachable" class="empty">
      {{ $t("gameInfo.historyUnavailableInGame") }}
    </div>
    <div v-else class="empty">{{ $t("career.empty") }}</div>
  </div>
</template>

<style scoped>
.matches {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  position: relative;
}
.match-list {
  flex: 1;
  min-height: 0;
  height: 100%;
}
.match-list :deep(.n-scrollbar-content) {
  padding-right: 2px;
}
.empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 11px;
  color: var(--text-dimmed, #9ca3af);
}
.spinner {
  width: 12px;
  height: 12px;
  border: 2px solid rgba(0, 0, 0, 0.1);
  border-top-color: #3b82f6;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
.mi {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  margin-bottom: 2px;
  padding: 0 6px 0 8px;
  border-radius: 4px;
  border-left: 3px solid transparent;
  flex-shrink: 0;
  transition: filter 0.12s ease;
  cursor: pointer;
}
.mi:hover {
  filter: brightness(1.06);
}
.mi-win {
  background: var(--win-bg, rgba(59, 130, 246, 0.14));
  border-left-color: var(--tier-blue, #3b82f6);
}
.mi-loss {
  background: var(--loss-bg, rgba(220, 38, 38, 0.17));
  border-left-color: var(--death-color, #dc2626);
}
.mi-remake {
  background: var(--hover-bg, rgba(156, 163, 175, 0.14));
  border-left-color: var(--text-dimmed, #9ca3af);
}
.mi-champ {
  width: 26px;
  height: 26px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
}
.mi-mid {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1px;
  line-height: 1.2;
}
.mi-mode {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-color, #111827);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mi-time {
  font-size: 10px;
  color: var(--text-dimmed, #9ca3af);
  display: flex;
  align-items: center;
  gap: 4px;
}
.mi-dur {
  color: var(--text-dimmed, #9ca3af);
  opacity: 0.85;
}
.mi-remake-tag {
  display: inline-flex;
  padding: 0 4px;
  border-radius: 2px;
  background: rgba(156, 163, 175, 0.25);
  color: #6b7280;
  font-size: 9px;
  font-weight: 700;
  line-height: 1.4;
}
.mi-kda {
  font-size: 12px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
  text-align: right;
  white-space: nowrap;
}
.mi-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  justify-content: center;
  gap: 0;
  flex-shrink: 0;
  line-height: 1.15;
}
.mi-cs {
  font-size: 9.5px;
  color: var(--text-dimmed, #9ca3af);
  font-variant-numeric: tabular-nums;
}
.mi-kda .k {
  color: var(--text-color, #111827);
}
.mi-kda .d {
  color: var(--death-color, #ef4444);
}
.mi-kda .a {
  color: var(--text-color, #111827);
}
.mi-kda .s {
  color: var(--text-dimmed, #9ca3af);
  margin: 0 1px;
}
</style>
