<script setup lang="ts">
import { NTag } from "naive-ui";
import {
  getDeckDisplayName,
  getTraitDisplayName,
  type TftMetaDeck,
} from "../../composables/useTftMetaDecks";
import {
  formatPercent,
  formatPlacement,
  getBadgeClass,
  getDeckBadges,
} from "../../utils/tftMetaDisplay";

const props = defineProps<{
  deck: TftMetaDeck;
  tier: string;
  selected: boolean;
}>();

const emit = defineEmits<{
  select: [deck: TftMetaDeck];
}>();

void props;
</script>

<template>
  <div
    :class="['comp-card', { selected }]"
    @click="emit('select', deck)"
  >
    <div class="card-header">
      <div :class="['tier-badge', (tier || 'c').toLowerCase()]">
        {{ tier }}
      </div>
      <div class="deck-name">{{ getDeckDisplayName(deck) }}</div>
      <div v-if="deck.cost != null" class="cost-badge">
        {{ deck.cost }}费
      </div>
    </div>

    <div class="stats-row">
      <div v-if="deck.stat?.win_rate != null" class="stat">
        <span class="stat-val win">{{ formatPercent(deck.stat.win_rate) }}</span>
        <span class="stat-lbl">{{ $t("tftPage.winRate") }}</span>
      </div>
      <div v-if="deck.stat?.top4_rate != null" class="stat">
        <span class="stat-val top4">{{ formatPercent(deck.stat.top4_rate) }}</span>
        <span class="stat-lbl">Top4</span>
      </div>
      <div v-if="deck.stat?.avg_placement != null" class="stat">
        <span class="stat-val place">{{ formatPlacement(deck.stat.avg_placement) }}</span>
        <span class="stat-lbl">{{ $t("tftPage.avgPlace") }}</span>
      </div>
    </div>

    <div v-if="getDeckBadges(deck).length" class="badges-row">
      <span
        v-for="(badge, bIdx) in getDeckBadges(deck)"
        :key="bIdx"
        :class="['badge-tag', getBadgeClass(badge)]"
      >
        {{ badge }}
      </span>
    </div>

    <div v-if="deck.traits?.length" class="traits-row">
      <n-tag
        v-for="tr in deck.traits.slice(0, 5)"
        :key="tr.key"
        size="tiny"
        :bordered="false"
        class="trait-tag"
      >
        {{ getTraitDisplayName(tr.key) }}
      </n-tag>
      <span v-if="deck.traits.length > 5" class="more-tag"
        >+{{ deck.traits.length - 5 }}</span
      >
    </div>
  </div>
</template>

<style scoped>
.comp-card {
  background: var(--card-bg, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.08));
  border-radius: 12px;
  padding: 12px;
  cursor: pointer;
  transition: transform 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
}
.comp-card:hover {
  transform: translateY(-2px);
  border-color: var(--primary-color, #10b981);
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.18);
}
.comp-card.selected {
  border-color: var(--primary-color, #10b981);
  box-shadow: 0 0 0 1px var(--primary-color, #10b981);
}
.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.tier-badge {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
  color: #fff;
  flex-shrink: 0;
}
.tier-badge.s {
  background: linear-gradient(135deg, #f59e0b 0%, #ef4444 100%);
}
.tier-badge.a {
  background: linear-gradient(135deg, #f97316 0%, #ea580c 100%);
}
.tier-badge.b {
  background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
}
.tier-badge.c {
  background: linear-gradient(135deg, #8b5cf6 0%, #7c3aed 100%);
}
.tier-badge.d {
  background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
}
.deck-name {
  font-weight: 700;
  color: var(--text-color);
  font-size: 0.95rem;
  line-height: 1.3;
  flex: 1;
  min-width: 0;
}
.cost-badge {
  font-size: 0.7rem;
  padding: 2px 6px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--text-muted);
  flex-shrink: 0;
}
.stats-row {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}
.stat {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex: 1;
  background: rgba(0, 0, 0, 0.2);
  padding: 4px 6px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.05);
}
.stat-val {
  font-weight: 800;
  font-size: 0.85rem;
  color: var(--text-color);
}
.stat-val.win {
  color: #10b981;
}
.stat-val.top4 {
  color: #3b82f6;
}
.stat-val.place {
  color: #f59e0b;
}
.stat-lbl {
  font-size: 0.63rem;
  color: var(--text-muted);
  margin-top: 1px;
}
.badges-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 8px;
}
.badge-tag {
  font-size: 0.68rem;
  padding: 2px 6px;
  border-radius: 4px;
  font-weight: 600;
}
.badge-reroll {
  background: rgba(59, 130, 246, 0.2);
  color: #93c5fd;
}
.badge-easy {
  background: rgba(16, 185, 129, 0.2);
  color: #6ee7b7;
}
.badge-hard {
  background: rgba(239, 68, 68, 0.2);
  color: #fca5a5;
}
.badge-honey {
  background: rgba(245, 158, 11, 0.2);
  color: #fcd34d;
}
.badge-default {
  background: rgba(0, 0, 0, 0.2);
  color: var(--text-muted);
}
.traits-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
}
.trait-tag {
  font-size: 0.7rem;
}
.more-tag {
  font-size: 0.7rem;
  color: var(--text-muted);
}
</style>
