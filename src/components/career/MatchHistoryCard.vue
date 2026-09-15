<script setup lang="ts">
import type { MatchDisplay } from "../../api/lcu";
import { NTooltip } from "naive-ui";
import LcuImage from "../LcuImage.vue";
import { useMatchDisplayHelpers } from "../../composables/matchDisplayHelpers";

defineProps<{
  match: MatchDisplay;
}>();

const emit = defineEmits<{
  click: [gameId: number];
}>();

const {
  formatTime,
  translateMapName,
  getQueueName,
  getResultText,
  getResultClass,
  getKdaClass,
  getSpellIcon,
} = useMatchDisplayHelpers();
</script>

<template>
  <div
    :class="['match-card', match.win ? 'win' : 'lose']"
    style="cursor: pointer"
    @click="emit('click', match.gameId)"
  >
          <!-- 1. 英雄头像、等级、技能、符文 -->
          <div class="champ-panel">
            <div class="champ-avatar-box">
              <LcuImage
                :src="match.championIconUrl"
                class="champ-avatar"
                alt="champ"
              />
              <div class="level-overlay">{{ match.champLevel }}</div>
            </div>
            <div class="spells-runes">
              <div class="spells-col">
                <div class="spell-slot">
                  <LcuImage
                    :src="getSpellIcon(match, 1)"
                    class="mini-icon"
                    alt="s1"
                  />
                </div>
                <div class="spell-slot">
                  <LcuImage
                    :src="getSpellIcon(match, 2)"
                    class="mini-icon"
                    alt="s2"
                  />
                </div>
              </div>
              <div v-if="match.queueId !== 2400 && match.queueId !== 2450" class="rune-slot">
                <LcuImage
                  :src="match.runeIconUrl"
                  class="mini-icon circular"
                  alt="rune"
                />
              </div>
            </div>
          </div>

          <!-- 2. 胜负状态与游戏模式/时长 -->
          <div class="result-panel">
            <span :class="['result-text', getResultClass(match)]">
              {{ getResultText(match) }}
            </span>
            <div class="result-sub-info">
              <span class="queue-mode">{{ getQueueName(match) }}</span>
              <span class="game-duration">{{ match.duration }}</span>
            </div>
          </div>

          <!-- 3. KDA 数字与文字 -->
          <div class="kda-panel">
            <div class="kda-numbers">
              <span class="bold">{{ match.kills }}</span> /
              <span class="bold death-red">{{ match.deaths }}</span> /
              <span class="bold">{{ match.assists }}</span>
            </div>
            <div class="kda-desc">
              <span class="kda-ratio" :class="getKdaClass(match.kda)"
                >{{ match.kda }} KDA</span
              >
            </div>
          </div>

          <!-- 4. 补刀数 (靠左) -->
          <div class="cs-panel" :title="'补刀: ' + match.cs">
            <svg class="cs-icon" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2a7 7 0 0 0-7 7v4.5c0 1.2.6 2.3 1.6 3l1.9 1.3v2.2a1 1 0 0 0 1.5.8l2-1.3 2 1.3a1 1 0 0 0 1.5-.8v-2.2l1.9-1.3c1-.7 1.6-1.8 1.6-3V9a7 7 0 0 0-7-7zm-3.5 8a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3zm7 0a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3z" />
            </svg>
            <span class="cs-count">{{ match.cs }}</span>
          </div>

          <!-- 5. 装备栏 (海克斯强化 + 前 6 件常规装备 + 第 7 件饰品) -->
          <div class="items-panel">
            <!-- 海克斯强化（仅海克斯大乱斗） -->
            <div v-if="(match.queueId === 2400 || match.queueId === 2450) && Boolean(match.augmentIconUrls?.length)" class="augment-grid">
              <n-tooltip
                v-for="(url, idx) in match.augmentIconUrls"
                :key="'aug-' + idx"
                trigger="hover"
                placement="top"
              >
                <template #trigger>
                  <div class="augment-slot">
                    <LcuImage :src="url" class="augment-img" alt="augment" />
                  </div>
                </template>
                <div class="career-aug-tooltip">
                  <div class="career-aug-tooltip-name">{{ match.augmentNames?.[idx] || "海克斯强化" }}</div>
                </div>
              </n-tooltip>
            </div>
            <div class="items-grid">
              <div v-for="idx in 6" :key="idx" class="item-slot">
                <LcuImage
                  v-if="match.itemIconUrls[idx - 1]"
                  :src="match.itemIconUrls[idx - 1]"
                  class="item-img"
                  alt="item"
                />
              </div>
            </div>
            <!-- 饰品独立显示 -->
            <div class="ward-slot">
              <LcuImage
                v-if="match.itemIconUrls[6]"
                :src="match.itemIconUrls[6]"
                class="item-img"
                alt="ward"
              />
            </div>
          </div>

          <!-- 6. 还原经济面板 (位于装备栏右侧) -->
          <div class="gold-panel" :title="'经济: ' + match.gold.toLocaleString() + ' 金币'">
            <span class="gold-count">{{ match.gold.toLocaleString() }}</span>
            <svg class="gold-icon" viewBox="0 0 24 24" fill="currentColor">
              <circle
                cx="12"
                cy="12"
                r="9"
                stroke="currentColor"
                stroke-width="1.8"
                fill="none"
              />
              <path
                d="M12 6.5v11M14.5 9.5H11.5a1.5 1.5 0 0 0 0 3h1a1.5 1.5 0 0 1 0 3H9.5"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
                fill="none"
              />
            </svg>
          </div>

          <!-- 7. 地图模式与日期 -->
          <div class="time-panel">
            <span class="game-map">{{ translateMapName(match.map) }}</span>
            <span class="match-time">{{ formatTime(match.timeStamp) }}</span>
          </div>
        </div>

</template>

<style scoped>
/* 战绩对局历史卡片 */
.match-history-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.match-card {
  display: flex;
  align-items: center;
  padding: 14px 20px;
  border-radius: 10px;
  border: 1px solid var(--border-color);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  cursor: pointer;
}

.match-card:hover {
  box-shadow: var(--shadow-md);
}

.match-card.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.match-card.win:hover {
  background-color: rgba(16, 185, 129, 0.12);
}

.match-card.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.match-card.lose:hover {
  background-color: rgba(239, 68, 68, 0.11);
}

/* 1. 英雄面板 */
.champ-panel {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 100px;
}

.champ-avatar-box {
  position: relative;
  width: 52px;
  height: 52px;
}

.champ-avatar {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  overflow: hidden;
  border: 1.5px solid var(--border-color);
}

.level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 18px;
  height: 18px;
  line-height: 16px;
  background-color: var(--card-bg);
  color: var(--text-color);
  border-radius: 50%;
  font-size: 0.7rem;
  font-weight: 700;
  text-align: center;
  border: 1px solid var(--border-color);
}

[data-theme="dark"] .level-overlay {
  background-color: var(--card-bg);
  color: var(--text-color);
  border-color: rgba(255, 255, 255, 0.15);
}

.spells-runes {
  display: flex;
  gap: 3px;
  align-items: center;
}

.spells-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spell-slot,
.rune-slot {
  width: 22px;
  height: 22px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.mini-icon {
  width: 100%;
  height: 100%;
  display: block;
}

.mini-icon.circular {
  border-radius: 50%;
}

/* 2. 胜负面板 */
.result-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 90px;
}

.result-text {
  font-size: 1rem;
  font-weight: 800;
}

.win-text {
  color: var(--win-color);
}

.win-text.gold-text {
  color: #f59e0b;
}

.lose-text {
  color: var(--loss-color);
}

.result-sub-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  margin-top: 2px;
}

.queue-mode {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.game-duration {
  font-size: 0.76rem;
  color: var(--text-dimmed);
  font-weight: 600;
}

/* 3. KDA面板 */
.kda-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 105px;
}

.kda-numbers {
  font-size: 1rem;
  color: var(--text-muted);
}

.bold {
  font-weight: 700;
  color: var(--text-color);
}

.kda-desc {
  margin-top: 2px;
}

.kda-ratio {
  color: var(--text-muted);
  font-size: 0.82rem;
  font-weight: 700;
}

.kda-perfect {
  color: #d97706;
}
.kda-great {
  color: #db2777;
}
.kda-good {
  color: #2563eb;
}
.kda-normal {
  color: var(--text-dimmed);
}

/* 4. 补刀与经济统计列 */
/* 4. 补刀面板 (靠左) */
.cs-panel {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 44px;
  margin-right: 6px;
}

.cs-count {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-muted);
}

.cs-icon {
  width: 14px;
  height: 14px;
  color: #94a3b8;
  flex-shrink: 0;
}

/* 6. 金币面板 (固定右侧，与最右侧时间面板保持固定 24px 间距) */
.gold-panel {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 62px;
  margin-left: auto;
  margin-right: 24px;
}

.gold-count {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-muted);
}

.gold-icon {
  width: 14px;
  height: 14px;
  color: #d97706;
  flex-shrink: 0;
}

/* 5. 装备面板 */
.items-panel {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.items-grid {
  display: flex;
  gap: 2px;
}

.item-slot,
.ward-slot {
  width: 28px;
  height: 28px;
  background-color: rgba(0, 0, 0, 0.04);
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.ward-slot {
  border-color: rgba(245, 158, 11, 0.3);
  background-color: rgba(245, 158, 11, 0.05);
  margin-left: 2px;
}

/* 海克斯强化图标 */
.augment-grid {
  display: flex;
  gap: 2px;
  margin-right: 4px;
}
.augment-slot {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(168, 85, 247, 0.45);
  background-color: rgba(147, 51, 234, 0.12);
  transition: all 0.2s ease;
  cursor: pointer;
}
.augment-slot:hover {
  border-color: #c084fc;
  box-shadow: 0 0 6px rgba(192, 132, 252, 0.6);
  transform: translateY(-1px) scale(1.05);
}
.augment-img {
  width: 100%;
  height: 100%;
  display: block;
}

/* 7. 时间面板 */
.time-panel {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  min-width: 80px;
  font-size: 0.82rem;
  color: var(--text-dimmed);
  white-space: nowrap;
  flex-shrink: 0;
}

.game-map,
.match-time {
  white-space: nowrap;
}

.map-name {
  font-weight: 600;
  color: var(--text-muted);
}

.match-time {
  margin-top: 4px;
}


</style>
