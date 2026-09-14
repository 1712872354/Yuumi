<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  getChampionIcon,
  PREMADE_COLORS,
  type PlayerData,
  type PremadePlayerLike,
} from "../../types/gameInfo";
import type { SavedPlayerMarker } from "../../api/lcu";
import { usePlayerSearch } from "../../composables/usePlayerSearch";
import LcuImage from "../LcuImage.vue";
import IronMedal from "../../assets/ranked-icons/iron.png";
import BronzeMedal from "../../assets/ranked-icons/bronze.png";
import SilverMedal from "../../assets/ranked-icons/silver.png";
import GoldMedal from "../../assets/ranked-icons/gold.png";
import PlatinumMedal from "../../assets/ranked-icons/platinum.png";
import EmeraldMedal from "../../assets/ranked-icons/emerald.png";
import DiamondMedal from "../../assets/ranked-icons/diamond.png";
import MasterMedal from "../../assets/ranked-icons/master.png";
import GrandmasterMedal from "../../assets/ranked-icons/grandmaster.png";
import ChallengerMedal from "../../assets/ranked-icons/challenger.png";

const RANKED_MEDAL_MAP: Record<string, string> = {
  IRON: IronMedal,
  BRONZE: BronzeMedal,
  SILVER: SilverMedal,
  GOLD: GoldMedal,
  PLATINUM: PlatinumMedal,
  EMERALD: EmeraldMedal,
  DIAMOND: DiamondMedal,
  MASTER: MasterMedal,
  GRANDMASTER: GrandmasterMedal,
  CHALLENGER: ChallengerMedal,
};

function getTierMedal(tier?: string | null): string | null {
  if (!tier) return null;
  return RANKED_MEDAL_MAP[tier.toUpperCase()] || null;
}

const props = defineProps<{
  player: PremadePlayerLike;
  playerData?: PlayerData;
  side: "ally" | "enemy";
  premadeIdx?: number;
  savedMap?: Record<string, SavedPlayerMarker>;
  selfPuuid?: string;
  index: number;
  matchFilter?: string;
}>();

const { t } = useI18n();
const { handleCareerClick } = usePlayerSearch();

// ─── 英雄 ID ───
const resolvedChampId = computed(() => {
  // 优先从 player 自身取，兜底从 playerData 的 championId 取
  if (props.player.championId && props.player.championId > 0) return props.player.championId;
  if ((props.player as Record<string, unknown>).botChampionId) return (props.player as Record<string, number>).botChampionId;
  if (props.playerData?.championId && props.playerData.championId > 0) return props.playerData.championId;
  // 召唤师头像兜底
  if (props.playerData?.info?.profileIconId) return 0; // 0 表示无英雄，走头像兜底
  return 0;
});

// ─── 召唤师信息 ───
const summonerInfo = computed(() => props.playerData?.info);
const displayName = computed(() => {
  if (summonerInfo.value?.gameName) return summonerInfo.value.gameName;
  if (summonerInfo.value?.displayName) return summonerInfo.value.displayName;
  // 从 player 上兜底
  const p = props.player as Record<string, unknown>;
  return (p.gameName as string) || (p.displayName as string) || (p.summonerName as string) || "";
});
const tagLine = computed(() => summonerInfo.value?.tagLine || "");

// ─── 段位 ───
const soloRank = computed(() => props.playerData?.ranked?.solo ?? null);
const flexRank = computed(() => props.playerData?.ranked?.flex ?? null);

const TIER_COLORS: Record<string, string> = {
  IRON: "#6b7280",
  BRONZE: "#b45309",
  SILVER: "#94a3b8",
  GOLD: "#d97706",
  PLATINUM: "#0d9488",
  EMERALD: "#059669",
  DIAMOND: "#3b82f6",
  MASTER: "#8b5cf6",
  GRANDMASTER: "#ef4444",
  CHALLENGER: "#f59e0b",
};

function getTierColor(tier?: string): string {
  if (!tier) return "var(--text-dimmed)";
  return TIER_COLORS[tier.toUpperCase()] || "var(--text-dimmed)";
}

// 短段位名（两字），对齐 LeagueAkari 风格
const SHORT_TIER_NAMES: Record<string, string> = {
  IRON: "黑铁",
  BRONZE: "黄铜",
  SILVER: "白银",
  GOLD: "黄金",
  PLATINUM: "铂金",
  EMERALD: "翡翠",
  DIAMOND: "钻石",
  MASTER: "大师",
  GRANDMASTER: "宗师",
  CHALLENGER: "王者",
};

function formatTierShort(entry: { tier: string; rank: string; leaguePoints?: number } | null): string {
  if (!entry || !entry.tier || entry.tier === "NA" || entry.tier === "NONE") return t("gameInfo.unranked");
  const tierKey = entry.tier.toUpperCase();
  const tierName = SHORT_TIER_NAMES[tierKey] || t(`tools.spoofTier.${tierKey}`);
  const highTier = ["MASTER", "GRANDMASTER", "CHALLENGER"].includes(tierKey);
  const lp = entry.leaguePoints !== undefined ? ` ${entry.leaguePoints}` : "";
  if (highTier) return `${tierName}${lp}`;
  if (!entry.rank || entry.rank === "NA") return `${tierName}${lp}`;
  return `${tierName} ${entry.rank}${lp}`;
}

// 主段位（用于边框色）：优先单双，次选灵活
const primaryTier = computed(() => {
  if (soloRank.value?.tier) return soloRank.value.tier;
  if (flexRank.value?.tier) return flexRank.value.tier;
  return "";
});

// ─── 组队颜色 ───
const premadeColor = computed(() => {
  if (props.premadeIdx === undefined || props.premadeIdx < 0) return null;
  const idx = props.premadeIdx % PREMADE_COLORS.length;
  return PREMADE_COLORS[idx];
});

// ─── 统计：胜率 / KDA / 位置 ───
const winRate = computed(() => props.playerData?.winRate);
const avgKda = computed(() => props.playerData?.avgKda);
const totalGames = computed(() => {
  const w = props.playerData?.winCount ?? 0;
  const l = props.playerData?.lossesCount ?? 0;
  return w + l;
});

function getWinRateColorClass(rate: number | undefined): string {
  if (rate === undefined) return "text-dimmed";
  if (rate >= 53) return "text-win";
  if (rate <= 47) return "text-loss";
  return "text-normal";
}

function getKdaColorClass(kda: number | undefined): string {
  if (kda === undefined) return "text-dimmed";
  if (kda >= 3) return "text-win";
  if (kda < 2) return "text-loss";
  return "text-normal";
}

// ─── 标签列表 ───
interface CardTag {
  text: string;
  class: string; // CSS class 名
  bgColor?: string;
  textColor?: string;
}

const cardTags = computed<CardTag[]>(() => {
  const tags: CardTag[] = [];
  const data = props.playerData;
  if (!data) return tags;

  // 1. 自己
  if (props.selfPuuid && data.info?.puuid && data.info.puuid === props.selfPuuid) {
    tags.push({ text: t("gameInfo.tagSelf"), class: "tag-self" });
  }

  // 2. 已标记
  const puuid = data.info?.puuid;
  if (puuid && props.savedMap?.[puuid]) {
    tags.push({ text: t("gameInfo.tagMarked"), class: "tag-marked" });
  }

  // 3. 宿命对局
  if (data.fateFlag === "ally") {
    tags.push({ text: t("gameInfo.tagFateAlly"), class: "tag-fate-ally" });
  } else if (data.fateFlag === "enemy") {
    tags.push({ text: t("gameInfo.tagFateEnemy"), class: "tag-fate-enemy" });
  }

  // 4. 组队
  if (props.premadeIdx !== undefined && props.premadeIdx >= 0 && premadeColor.value) {
    // 计算组内人数：通过 premadeIdx 反推（这里简化显示"组"）
    tags.push({
      text: t("gameInfo.tagPremade", { size: "组" }),
      class: "tag-premade",
      bgColor: premadeColor.value.dot,
      textColor: "#fff",
    });
  }

  // 5. 连胜 / 连败
  if (data.streak) {
    if (data.streak.type === "win" && data.streak.count >= 2) {
      tags.push({ text: t("gameInfo.tagWinStreak", { count: data.streak.count }), class: "tag-streak-win" });
    } else if (data.streak.type === "loss" && data.streak.count >= 2) {
      tags.push({ text: t("gameInfo.tagLoseStreak", { count: data.streak.count }), class: "tag-streak-loss" });
    }
  }

  // 6. 高胜率
  if (data.winRate !== undefined && data.winRate >= 55 && totalGames.value >= 10) {
    tags.push({ text: t("gameInfo.tagHighWinRate"), class: "tag-high-wr" });
  }

  return tags;
});

// ─── 擅长英雄（前 9 个） ───
const topMasteries = computed(() => {
  if (!props.playerData?.masteries) return [];
  return props.playerData.masteries.slice(0, 9);
});

// ─── 战绩列表 ───
const matches = computed(() => {
  if (!props.playerData?.matches) return [];
  return props.playerData.matches;
});

// ─── 战绩隐藏 ───
const isMatchHidden = computed(() => props.playerData?.matchHistoryHidden === true);

// ─── 加载中 ───
const isLoading = computed(() => props.playerData?.loading === true);

// ─── 格式化时间（用 MatchDisplay.time 字段） ───
function formatMatchTime(timeStr: string): string {
  // time 字段通常是 "X分钟前" / "X小时前" / "X天前" 等已格式化字符串
  return timeStr || "";
}

// ─── 点击召唤师名 ───
function onSummonerClick(e: MouseEvent) {
  if (!summonerInfo.value) return;
  const player = {
    gameName: summonerInfo.value.gameName,
    tagLine: summonerInfo.value.tagLine,
    puuid: summonerInfo.value.puuid,
    summonerId: summonerInfo.value.summonerId,
  };
  handleCareerClick(e, player, props.playerData);
}
</script>

<template>
  <div
    class="player-info-card-v2"
    :class="side"
    :style="{
      borderColor: premadeColor?.border || getTierColor(primaryTier || undefined),
    }"
  >
    <!-- 组队三角装饰 -->
    <div
      v-if="premadeColor"
      class="premade-triangle"
      :style="{ backgroundColor: premadeColor.dot }"
    ></div>

    <!-- ─── 区域 1：头部 ─── -->
    <div class="card-header">
      <!-- 头像 -->
      <div class="avatar-wrap" @click="onSummonerClick">
        <div class="avatar-ring">
          <LcuImage
            v-if="resolvedChampId > 0"
            :src="getChampionIcon(resolvedChampId)"
            class="avatar-img"
          />
          <div v-else class="avatar-img avatar-placeholder">?</div>
        </div>
        <div v-if="summonerInfo?.summonerLevel" class="level-badge">
          {{ summonerInfo.summonerLevel }}
        </div>
        <img
          v-if="getTierMedal(primaryTier)"
          :src="getTierMedal(primaryTier)!"
          class="tier-medal"
          :alt="primaryTier"
        />
      </div>

      <!-- 信息区 -->
      <div class="header-info">
        <!-- 名字行 -->
        <div class="name-row">
          <span
            class="summoner-name"
            :style="{ color: premadeColor?.dot || '' }"
            @click="onSummonerClick"
          >
            {{ displayName || '—' }}
          </span>
          <span v-if="tagLine" class="tag-line">#{{ tagLine }}</span>
        </div>

        <!-- 段位区（单行：单双 + 灵活左右并排） -->
        <div class="tier-row">
          <!-- 单双 -->
          <div
            class="tier-item"
            :style="{ color: getTierColor(soloRank?.tier) }"
            :title="soloRank ? formatTierShort(soloRank) : t('gameInfo.soloRank') + ' ' + t('gameInfo.unranked')"
          >
            <img
              v-if="getTierMedal(soloRank?.tier)"
              :src="getTierMedal(soloRank?.tier)!"
              class="tier-item-icon"
              alt=""
            />
            <span class="tier-item-text">
              {{ soloRank ? formatTierShort(soloRank) : t("gameInfo.unranked") }}
            </span>
          </div>
          <!-- 灵活 -->
          <div
            class="tier-item tier-item-flex"
            :style="{ color: getTierColor(flexRank?.tier) }"
            :title="flexRank ? formatTierShort(flexRank) : t('gameInfo.flexRank') + ' ' + t('gameInfo.unranked')"
          >
            <img
              v-if="getTierMedal(flexRank?.tier)"
              :src="getTierMedal(flexRank?.tier)!"
              class="tier-item-icon"
              alt=""
            />
            <span class="tier-item-text tier-item-text-flex">
              {{ flexRank ? formatTierShort(flexRank) : t("gameInfo.unranked") }}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- ─── 区域 2：统计区（三等分） ─── -->
    <div class="card-stats">
      <div class="stat-col">
        <span class="stat-value" :class="getWinRateColorClass(winRate)">
          {{ winRate !== undefined ? `${winRate}%` : '—' }}
        </span>
        <span class="stat-label">{{ t("gameInfo.teamWinRate") }}</span>
      </div>
      <div class="stat-col">
        <span class="stat-value" :class="getKdaColorClass(avgKda)">
          {{ avgKda !== undefined ? avgKda.toFixed(2) : '—' }}
        </span>
        <span class="stat-label">{{ t("career.kda") }}</span>
      </div>
      <div class="stat-col">
        <span class="stat-value text-dimmed">—</span>
        <span class="stat-label">{{ t("gameInfo.position") }}</span>
      </div>
    </div>

    <!-- ─── 区域 3：标签区 ─── -->
    <div v-if="cardTags.length" class="card-tags">
      <span
        v-for="(tag, i) in cardTags"
        :key="i"
        class="card-tag"
        :class="tag.class"
        :style="{
          backgroundColor: tag.bgColor || '',
          color: tag.textColor || '',
        }"
      >
        {{ tag.text }}
      </span>
    </div>

    <!-- ─── 区域 4：擅长英雄 ─── -->
    <div v-if="topMasteries.length" class="card-mastery">
      <div
        v-for="m in topMasteries"
        :key="m.championId"
        class="mastery-icon-wrap"
        :title="`${m.championPoints.toLocaleString()} 点`"
      >
        <LcuImage
          :src="getChampionIcon(m.championId)"
          class="mastery-icon"
        />
        <span v-if="m.championLevel >= 5" class="mastery-star">★</span>
      </div>
    </div>

    <!-- ─── 区域 5：战绩列表 ─── -->
    <div class="card-matches">
      <template v-if="isLoading">
        <div class="match-empty">
          <div class="loading-spinner"></div>
          <span>{{ t("career.loading") }}</span>
        </div>
      </template>

      <template v-else-if="isMatchHidden">
        <div class="match-empty">
          <span class="hidden-icon">🔒</span>
          <span>战绩已隐藏</span>
        </div>
      </template>

      <template v-else-if="matches.length === 0">
        <div class="match-empty">
          <span>{{ t("career.empty") }}</span>
        </div>
      </template>

      <template v-else>
        <div
          v-for="match in matches"
          :key="match.gameId"
          class="match-item"
          :class="{
            'match-win': match.win === true,
            'match-loss': match.win === false,
            'match-remake': match.win === null || match.remake,
          }"
        >
          <LcuImage
            :src="getChampionIcon(match.championId)"
            class="match-champ"
          />
          <div class="match-info">
            <span class="match-mode">{{ match.name || '' }}</span>
            <span class="match-time">{{ formatMatchTime(match.shortTime || match.time) }}</span>
          </div>
          <div class="match-kda">
            <span class="kda-kill">{{ match.kills }}</span>
            <span class="kda-slash">/</span>
            <span class="kda-death">{{ match.deaths }}</span>
            <span class="kda-slash">/</span>
            <span class="kda-assist">{{ match.assists }}</span>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.player-info-card-v2 {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border-radius: 8px;
  border-width: 2px;
  border-style: solid;
  background: rgba(255, 255, 255, 0.55);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  transition: transform 0.2s ease, box-shadow 0.2s ease, filter 0.2s ease;
  overflow: hidden;
  min-height: 0;
}

.player-info-card-v2:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
  filter: brightness(1.03);
}

.player-info-card-v2.ally {
  background: rgba(59, 130, 246, 0.06);
}
.player-info-card-v2.enemy {
  background: rgba(244, 63, 94, 0.06);
}

/* 组队三角装饰 */
.premade-triangle {
  position: absolute;
  top: 0;
  right: 0;
  width: 18px;
  height: 18px;
  transform: translateX(50%) translateY(-50%) rotate(45deg);
  z-index: 1;
  box-shadow: -1px 1px 2px rgba(0, 0, 0, 0.15);
}

/* ─── 头部 ─── */
.card-header {
  display: flex;
  gap: 8px;
  align-items: stretch;
  flex-shrink: 0;
}

.avatar-wrap {
  position: relative;
  cursor: pointer;
  flex-shrink: 0;
}
.avatar-ring {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  padding: 2px;
  background: rgba(255, 255, 255, 0.4);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.6);
  transition: filter 0.15s ease;
}
.avatar-wrap:hover .avatar-ring {
  filter: brightness(1.1);
}
.avatar-img {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  display: block;
}
.avatar-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-card, rgba(255, 255, 255, 0.3));
  color: var(--text-dimmed);
  font-size: 14px;
  font-weight: 800;
}
.level-badge {
  position: absolute;
  right: -2px;
  bottom: -2px;
  min-width: 18px;
  height: 16px;
  padding: 0 4px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.65);
  color: #fff;
  font-size: 10px;
  font-weight: 600;
  line-height: 16px;
  text-align: center;
  white-space: nowrap;
}

.tier-medal {
  position: absolute;
  left: -4px;
  bottom: -6px;
  width: 22px;
  height: 22px;
  object-fit: contain;
  filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.4));
  pointer-events: none;
}

.header-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
}

.name-row {
  display: flex;
  align-items: baseline;
  gap: 4px;
  min-width: 0;
}
.summoner-name {
  font-size: 13px;
  font-weight: 800;
  color: var(--text-color);
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: filter 0.15s ease;
}
.summoner-name:hover {
  filter: brightness(1.2);
}
.tag-line {
  font-size: 11px;
  color: var(--text-dimmed);
  flex-shrink: 0;
}

/* ─── 段位区（两行） ─── */
/* ─── 段位区（单行左右并排） ─── */
.tier-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.tier-item {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  overflow: hidden;
}
.tier-item-icon {
  flex-shrink: 0;
  width: 14px;
  height: 14px;
  object-fit: contain;
}
.tier-item-text {
  font-size: 11px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: 0.2px;
}
.tier-item-text-flex {
  opacity: 0.85;
}

/* ─── 统计区 ─── */
.card-stats {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  padding: 4px 0;
  border-top: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
  border-bottom: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
}
.stat-col {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
  line-height: 1.2;
}
.stat-value {
  font-size: 13px;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
}
.stat-label {
  font-size: 9px;
  color: var(--text-dimmed);
  font-weight: 500;
}
.text-win {
  color: #059669;
}
.text-loss {
  color: #dc2626;
}
.text-normal {
  color: var(--text-color);
}
.text-dimmed {
  color: var(--text-dimmed);
}

/* ─── 标签区 ─── */
.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
  flex-shrink: 0;
}
.card-tag {
  display: inline-flex;
  align-items: center;
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 600;
  line-height: 1.5;
  white-space: nowrap;
}
.tag-self {
  background: rgba(59, 130, 246, 0.15);
  color: #3b82f6;
}
.tag-marked {
  background: rgba(5, 150, 105, 0.15);
  color: #059669;
}
.tag-fate-ally {
  background: rgba(5, 150, 105, 0.15);
  color: #059669;
}
.tag-fate-enemy {
  background: rgba(220, 38, 38, 0.12);
  color: #dc2626;
}
.tag-premade {
  color: #fff;
}
.tag-streak-win {
  background: rgba(245, 158, 11, 0.15);
  color: #d97706;
}
.tag-streak-loss {
  background: rgba(220, 38, 38, 0.12);
  color: #dc2626;
}
.tag-high-wr {
  background: rgba(5, 150, 105, 0.15);
  color: #059669;
}

/* ─── 擅长英雄 ─── */
.card-mastery {
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
  flex-shrink: 0;
}
.mastery-icon-wrap {
  position: relative;
  width: 22px;
  height: 22px;
  border-radius: 3px;
  overflow: hidden;
  cursor: default;
}
.mastery-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.mastery-star {
  position: absolute;
  right: -1px;
  bottom: -2px;
  font-size: 10px;
  color: #facc15;
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.6);
  line-height: 1;
}

/* ─── 战绩列表 ─── */
.card-matches {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding-right: 2px;
}
.card-matches::-webkit-scrollbar {
  width: 4px;
}
.card-matches::-webkit-scrollbar-track {
  background: transparent;
}
.card-matches::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.15);
  border-radius: 2px;
}

.match-item {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 3px 6px;
  border-radius: 4px;
  border-left: 3px solid transparent;
  flex-shrink: 0;
  transition: filter 0.15s ease;
}
.match-item:hover {
  filter: brightness(1.05);
}
.match-win {
  background: rgba(59, 130, 246, 0.14);
  border-left-color: #3b82f6;
  border-left-width: 3px;
}
.match-loss {
  background: rgba(220, 38, 38, 0.18);
  border-left-color: #dc2626;
  border-left-width: 3px;
}
.match-remake {
  background: rgba(156, 163, 175, 0.15);
  border-left-color: #9ca3af;
}

.match-champ {
  width: 24px;
  height: 24px;
  border-radius: 3px;
  object-fit: cover;
  flex-shrink: 0;
}
.match-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1px;
  line-height: 1.2;
}
.match-mode {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.match-time {
  font-size: 10px;
  color: var(--text-dimmed);
}
.match-kda {
  font-size: 11px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--text-color);
  flex-shrink: 0;
  line-height: 1.2;
  text-align: right;
}
.kda-kill { color: var(--text-color); }
.kda-death { color: #ef4444; }
.kda-assist { color: var(--text-color); }
.kda-slash {
  color: var(--text-dimmed);
  margin: 0 1px;
}

/* 空状态 */
.match-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--text-dimmed);
  font-size: 11px;
}
.hidden-icon {
  font-size: 18px;
}

/* loading spinner */
.loading-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(0, 0, 0, 0.1);
  border-top-color: var(--primary-color, #3b82f6);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
