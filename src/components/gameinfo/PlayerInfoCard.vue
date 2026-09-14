<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { NVirtualList } from "naive-ui";
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
  const key = tier.toUpperCase();
  if (key === "NONE" || key === "NA") return null;
  return RANKED_MEDAL_MAP[key] || null;
}

const props = defineProps<{
  player: PremadePlayerLike;
  playerData?: PlayerData;
  side: "ally" | "enemy";
  premadeIdx?: number;
  premadeSize?: number;
  savedMap?: Record<string, SavedPlayerMarker>;
  selfPuuid?: string;
  index: number;
}>();

const { t } = useI18n();
const { handleCareerClick } = usePlayerSearch();

const POSITION_LABELS: Record<string, string> = {
  TOP: "上单",
  JUNGLE: "打野",
  MIDDLE: "中单",
  BOTTOM: "下路",
  UTILITY: "辅助",
  NONE: "",
  INVALID: "",
};

const assignedPosition = computed(() => {
  const raw = props.player.assignedPosition;
  if (!raw || raw === "NONE" || raw === "INVALID") return "";
  return POSITION_LABELS[raw.toUpperCase()] || raw;
});

const resolvedChampId = computed(() => {
  if (props.player.championId && props.player.championId > 0) return props.player.championId;
  const botChamp = (props.player as Record<string, number>).botChampionId;
  if (botChamp && botChamp > 0) return botChamp;
  if (props.playerData?.championId && props.playerData.championId > 0) {
    return props.playerData.championId;
  }
  return 0;
});

const summonerInfo = computed(() => props.playerData?.info);
const displayName = computed(() => {
  if (summonerInfo.value?.gameName) return summonerInfo.value.gameName;
  if (summonerInfo.value?.displayName) return summonerInfo.value.displayName;
  const p = props.player as Record<string, unknown>;
  return (p.gameName as string) || (p.displayName as string) || (p.summonerName as string) || "";
});
const tagLine = computed(() => summonerInfo.value?.tagLine || "");

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
  if (!tier) return "var(--text-dimmed, #6b7280)";
  const key = tier.toUpperCase();
  if (key === "NONE" || key === "NA") return "var(--text-dimmed, #6b7280)";
  return TIER_COLORS[key] || "var(--text-dimmed, #6b7280)";
}

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

function formatTierShort(
  entry: { tier: string; rank: string; leaguePoints?: number } | null,
): string {
  if (!entry || !entry.tier || entry.tier === "NA" || entry.tier === "NONE") {
    return t("gameInfo.unranked");
  }
  const tierKey = entry.tier.toUpperCase();
  const tierName = SHORT_TIER_NAMES[tierKey] || t(`tools.spoofTier.${tierKey}`);
  const highTier = ["MASTER", "GRANDMASTER", "CHALLENGER"].includes(tierKey);
  const lp = entry.leaguePoints !== undefined ? ` ${entry.leaguePoints}` : "";
  if (highTier) return `${tierName}${lp}`;
  if (!entry.rank || entry.rank === "NA") return `${tierName}${lp}`;
  return `${tierName}${entry.rank}${lp}`;
}

const primaryTier = computed(() => {
  if (soloRank.value?.tier && soloRank.value.tier !== "NONE") return soloRank.value.tier;
  if (flexRank.value?.tier && flexRank.value.tier !== "NONE") return flexRank.value.tier;
  return "";
});

const premadeColor = computed(() => {
  if (props.premadeIdx === undefined || props.premadeIdx < 0) return null;
  return PREMADE_COLORS[props.premadeIdx % PREMADE_COLORS.length];
});

const winCount = computed(() => props.playerData?.winCount ?? 0);
const lossCount = computed(() => props.playerData?.lossesCount ?? 0);
const totalGames = computed(() => winCount.value + lossCount.value);
const winRate = computed(() => props.playerData?.winRate);
const avgKda = computed(() => props.playerData?.avgKda);

const avgCs = computed(() => {
  const list = props.playerData?.matches;
  if (!list || list.length === 0) return undefined;
  const real = list.filter((m) => !m.remake);
  if (real.length === 0) return undefined;
  const sum = real.reduce((acc, m) => acc + (m.cs || 0), 0);
  return Math.round(sum / real.length);
});

function getWinRateClass(rate: number | undefined): string {
  if (rate === undefined) return "stat-dim";
  if (rate >= 53) return "stat-win";
  if (rate <= 47) return "stat-loss";
  return "stat-normal";
}

function getKdaClass(kda: number | undefined): string {
  if (kda === undefined) return "stat-dim";
  if (kda >= 3) return "stat-win";
  if (kda < 2) return "stat-loss";
  return "stat-normal";
}

const streakBadge = computed(() => {
  const s = props.playerData?.streak;
  if (!s || s.count < 2) return null;
  return s.type === "win"
    ? { text: `${s.count}连胜`, cls: "badge-win" }
    : { text: `${s.count}连败`, cls: "badge-loss" };
});

const cardTags = computed(() => {
  const tags: { text: string; cls: string; bg?: string; color?: string }[] = [];
  const data = props.playerData;
  if (!data) return tags;

  if (props.selfPuuid && data.info?.puuid && data.info.puuid === props.selfPuuid) {
    tags.push({ text: t("gameInfo.tagSelf"), cls: "tag-self" });
  }
  const puuid = data.info?.puuid;
  if (puuid && props.savedMap?.[puuid]) {
    tags.push({ text: t("gameInfo.tagMarked"), cls: "tag-marked" });
  }
  if (data.fateFlag === "ally") {
    tags.push({ text: t("gameInfo.tagFateAlly"), cls: "tag-fate-ally" });
  } else if (data.fateFlag === "enemy") {
    tags.push({ text: t("gameInfo.tagFateEnemy"), cls: "tag-fate-enemy" });
  }
  if (props.premadeIdx !== undefined && props.premadeIdx >= 0 && premadeColor.value) {
    const size = props.premadeSize && props.premadeSize >= 2 ? props.premadeSize : null;
    tags.push({
      text: size
        ? t("gameInfo.tagPremadeN", { count: size })
        : t("gameInfo.tagPremade", { size: "组" }),
      cls: "tag-premade",
      bg: premadeColor.value.dot,
      color: "#fff",
    });
  }
  if (assignedPosition.value) {
    tags.push({ text: `当前:${assignedPosition.value}`, cls: "tag-pos" });
  }
  if (data.winRate !== undefined && data.winRate >= 55 && totalGames.value >= 10) {
    tags.push({ text: t("gameInfo.tagHighWinRate"), cls: "tag-high-wr" });
  }
  return tags;
});

const topMasteries = computed(() => {
  if (!props.playerData?.masteries) return [];
  return props.playerData.masteries.slice(0, 9);
});

const matches = computed(() => {
  if (!props.playerData?.matches) return [];
  return props.playerData.matches;
});

const isMatchHidden = computed(() => props.playerData?.matchHistoryHidden === true);
const isLoading = computed(() => props.playerData?.loading === true);

function onSummonerClick(e: MouseEvent) {
  if (!summonerInfo.value) return;
  handleCareerClick(
    e,
    {
      displayName: displayName.value,
      tagLine: summonerInfo.value.tagLine,
      puuid: summonerInfo.value.puuid,
    },
    props.playerData,
  );
}

function copyName(e: MouseEvent) {
  e.stopPropagation();
  const text = tagLine.value ? `${displayName.value}#${tagLine.value}` : displayName.value;
  if (!text) return;
  navigator.clipboard?.writeText(text).catch(() => {});
}

function copyGameId(e: MouseEvent, gameId: number) {
  e.stopPropagation();
  if (!gameId) return;
  navigator.clipboard?.writeText(String(gameId)).catch(() => {});
}
</script>

<template>
  <div
    class="pic"
    :class="side"
    :style="{
      borderColor: premadeColor?.border || getTierColor(primaryTier || undefined),
    }"
  >
    <div
      v-if="premadeColor"
      class="premade-corner"
      :style="{ backgroundColor: premadeColor.dot }"
    ></div>

    <!-- 头部：头像 + 名字 + 段位 -->
    <div class="pic-header">
      <div class="avatar-wrap" @click="onSummonerClick">
        <LcuImage
          v-if="resolvedChampId > 0"
          :src="getChampionIcon(resolvedChampId)"
          class="avatar-img"
        />
        <LcuImage
          v-else-if="summonerInfo?.profileIconId"
          :src="`/lol-game-data/assets/v1/profile-icons/${summonerInfo.profileIconId}.jpg`"
          class="avatar-img"
        />
        <div v-else class="avatar-img avatar-ph">?</div>
        <span v-if="summonerInfo?.summonerLevel" class="lvl">{{ summonerInfo.summonerLevel }}</span>
      </div>

      <div class="head-meta">
        <div class="name-row">
          <span class="pname" :style="{ color: premadeColor?.dot || '' }" @click="onSummonerClick">
            {{ displayName || "—" }}
          </span>
          <span v-if="tagLine" class="ptag">#{{ tagLine }}</span>
          <button class="copy-btn" :title="'复制'" @click="copyName">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
          </button>
        </div>

        <div class="rank-row">
          <div class="rank-item" :style="{ color: getTierColor(soloRank?.tier) }">
            <img v-if="getTierMedal(soloRank?.tier)" :src="getTierMedal(soloRank?.tier)!" class="rank-medal" alt="" />
            <span class="rank-label">{{ $t("gameInfo.soloRank") }}:</span>
            <span class="rank-val">{{ formatTierShort(soloRank) }}</span>
          </div>
          <div class="rank-item" :style="{ color: getTierColor(flexRank?.tier) }">
            <img v-if="getTierMedal(flexRank?.tier)" :src="getTierMedal(flexRank?.tier)!" class="rank-medal" alt="" />
            <span class="rank-label">{{ $t("gameInfo.flexRank") }}:</span>
            <span class="rank-val">{{ formatTierShort(flexRank) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 擅长英雄 -->
    <div v-if="topMasteries.length" class="mastery-row">
      <span class="mastery-label">{{ $t("gameInfo.mastery", "擅长") }}:</span>
      <div
        v-for="m in topMasteries"
        :key="m.championId"
        class="mastery-icon-wrap"
        :title="`${m.championPoints.toLocaleString()} 点`"
      >
        <LcuImage :src="getChampionIcon(m.championId)" class="mastery-icon" />
        <span v-if="m.championLevel >= 7" class="mastery-star">★</span>
      </div>
    </div>

    <!-- 统计区：内联 标签:值 -->
    <div class="stats-block">
      <div class="stats-line">
        <span class="si">
          <span class="si-l">{{ $t("gameInfo.wins", "胜场") }}:</span>
          <span class="si-v">{{ winCount }}/{{ totalGames || "—" }}</span>
        </span>
        <span class="si">
          <span class="si-l">{{ $t("gameInfo.teamWinRate") }}:</span>
          <span class="si-v" :class="getWinRateClass(winRate)">
            {{ winRate !== undefined ? `${winRate}%` : "—" }}
          </span>
        </span>
        <span v-if="streakBadge" class="streak" :class="streakBadge.cls">{{ streakBadge.text }}</span>
      </div>
      <div class="stats-line">
        <span class="si">
          <span class="si-l">KDA:</span>
          <span class="si-v" :class="getKdaClass(avgKda)">
            {{ avgKda !== undefined ? avgKda.toFixed(1) : "—" }}
          </span>
        </span>
        <span v-if="avgCs !== undefined" class="si">
          <span class="si-l">CS:</span>
          <span class="si-v">{{ avgCs }}</span>
        </span>
      </div>
      <div v-if="cardTags.length" class="tag-row">
        <span
          v-for="(tag, i) in cardTags"
          :key="i"
          class="tag"
          :class="tag.cls"
          :style="{ backgroundColor: tag.bg || '', color: tag.color || '' }"
        >
          {{ tag.text }}
        </span>
      </div>
    </div>

    <!-- 战绩列表（虚拟滚动） -->
    <div class="matches">
      <div v-if="isLoading" class="empty">
        <span class="spinner"></span>
        <span>{{ $t("career.loading") }}</span>
      </div>
      <div v-else-if="isMatchHidden" class="empty">🔒 {{ $t("gameInfo.matchHidden", "战绩已隐藏") }}</div>
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
      <div v-else class="empty">{{ $t("career.empty") }}</div>
    </div>
  </div>
</template>

<style scoped>
.pic {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 8px 9px 7px;
  border-radius: 8px;
  border: 2px solid transparent;
  background: var(--card-bg, rgba(255, 255, 255, 0.65));
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  box-shadow: var(--shadow-sm, 0 1px 6px rgba(0, 0, 0, 0.07));
  overflow: hidden;
  height: 100%;
  min-height: 0;
  transition: filter 0.15s ease, box-shadow 0.15s ease;
}
.pic:hover {
  filter: brightness(1.03);
  box-shadow: var(--shadow-md, 0 2px 10px rgba(0, 0, 0, 0.1));
}
.pic.ally {
  background: color-mix(in srgb, var(--tier-blue, #3b82f6) 6%, var(--card-bg, rgba(255, 255, 255, 0.65)));
}
.pic.enemy {
  background: color-mix(in srgb, #f43f5e 6%, var(--card-bg, rgba(255, 255, 255, 0.65)));
}

.premade-corner {
  position: absolute;
  top: 0;
  right: 0;
  width: 14px;
  height: 14px;
  transform: translate(50%, -50%) rotate(45deg);
  z-index: 1;
}

/* ─── 头部 ─── */
.pic-header {
  display: flex;
  gap: 7px;
  flex-shrink: 0;
}
.avatar-wrap {
  position: relative;
  flex-shrink: 0;
  cursor: pointer;
  width: 42px;
  height: 42px;
}
.avatar-img {
  width: 42px;
  height: 42px;
  border-radius: 50%;
  object-fit: cover;
  border: 1.5px solid rgba(255, 255, 255, 0.55);
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.08);
  display: block;
}
.avatar-ph {
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.08);
  color: var(--text-dimmed);
  font-weight: 800;
  font-size: 14px;
}
.lvl {
  position: absolute;
  right: -3px;
  bottom: -2px;
  min-width: 16px;
  padding: 0 3px;
  height: 14px;
  border-radius: 7px;
  background: rgba(0, 0, 0, 0.7);
  color: #fff;
  font-size: 9px;
  font-weight: 700;
  line-height: 14px;
  text-align: center;
}

.head-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
}

.name-row {
  display: flex;
  align-items: center;
  gap: 3px;
  min-width: 0;
}
.pname {
  font-size: 12.5px;
  font-weight: 800;
  color: var(--text-color, #111827);
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 70%;
}
.pname:hover {
  filter: brightness(1.2);
}
.ptag {
  font-size: 10px;
  color: var(--text-dimmed, #9ca3af);
  flex-shrink: 0;
}
.copy-btn {
  margin-left: auto;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border: none;
  background: transparent;
  color: var(--text-dimmed, #9ca3af);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 3px;
  padding: 0;
}
.copy-btn:hover {
  background: rgba(0, 0, 0, 0.08);
  color: var(--text-color);
}

.rank-row {
  display: flex;
  gap: 6px;
  min-width: 0;
}
.rank-item {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 10.5px;
  overflow: hidden;
  white-space: nowrap;
}
.rank-medal {
  width: 13px;
  height: 13px;
  object-fit: contain;
  flex-shrink: 0;
}
.rank-label {
  color: var(--text-dimmed, #9ca3af);
  flex-shrink: 0;
}
.rank-val {
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ─── 擅长 ─── */
.mastery-row {
  display: flex;
  align-items: center;
  gap: 3px;
  flex-shrink: 0;
  flex-wrap: wrap;
}
.mastery-label {
  font-size: 10px;
  color: var(--text-dimmed, #9ca3af);
  font-weight: 600;
  flex-shrink: 0;
}
.mastery-icon-wrap {
  position: relative;
  width: 18px;
  height: 18px;
}
.mastery-icon {
  width: 18px;
  height: 18px;
  border-radius: 3px;
  object-fit: cover;
  display: block;
}
.mastery-star {
  position: absolute;
  right: -2px;
  bottom: -3px;
  font-size: 8px;
  color: #facc15;
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.7);
  line-height: 1;
}

/* ─── 统计 ─── */
.stats-block {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex-shrink: 0;
  padding: 4px 0;
  border-top: 1px solid var(--border-color, rgba(0, 0, 0, 0.06));
  border-bottom: 1px solid var(--border-color, rgba(0, 0, 0, 0.06));
}
.stats-line {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  font-size: 11px;
  line-height: 1.35;
}
.si {
  display: inline-flex;
  align-items: baseline;
  gap: 2px;
  white-space: nowrap;
}
.si-l {
  color: var(--text-dimmed, #9ca3af);
  font-weight: 500;
}
.si-v {
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  color: var(--text-color, #111827);
}
.stat-win {
  color: var(--win-color, #059669);
}
.stat-loss {
  color: var(--loss-color, #dc2626);
}
.stat-normal {
  color: var(--text-color, #111827);
}
.stat-dim {
  color: var(--text-dimmed, #9ca3af);
}

.streak {
  display: inline-flex;
  align-items: center;
  padding: 0 5px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 700;
  line-height: 1.5;
  margin-left: auto;
}
.badge-win {
  background: var(--win-bg, rgba(5, 150, 105, 0.15));
  color: var(--win-color, #059669);
}
.badge-loss {
  background: var(--loss-bg, rgba(220, 38, 38, 0.15));
  color: var(--loss-color, #dc2626);
}

.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
  margin-top: 1px;
}
.tag {
  display: inline-flex;
  align-items: center;
  padding: 0 5px;
  border-radius: 3px;
  font-size: 9.5px;
  font-weight: 700;
  line-height: 1.55;
  background: rgba(0, 0, 0, 0.06);
  color: var(--text-color);
}
.tag-self {
  background: rgba(59, 130, 246, 0.18);
  color: #2563eb;
}
.tag-marked {
  background: rgba(168, 85, 247, 0.18);
  color: #9333ea;
}
.tag-fate-ally {
  background: rgba(5, 150, 105, 0.15);
  color: #059669;
}
.tag-fate-enemy {
  background: rgba(220, 38, 38, 0.12);
  color: #dc2626;
}
.tag-high-wr {
  background: rgba(245, 158, 11, 0.18);
  color: #d97706;
}
.tag-pos {
  background: var(--tier-blue-bg, rgba(59, 130, 246, 0.12));
  color: var(--tier-blue, #3b82f6);
}

/* ─── 战绩 ─── */
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
