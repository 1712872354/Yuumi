<script setup lang="ts">
import { computed, inject, h, ref } from "vue";
import { useI18n } from "vue-i18n";
import { NInput, useDialog } from "naive-ui";
import { useLcuStore } from "../../store/lcuStore";
import {
  getChampionIcon,
  PREMADE_COLORS,
  type PlayerData,
  type PremadePlayerLike,
} from "../../types/gameInfo";
import type { SavedPlayerMarker } from "../../api/lcu";
import { setPlayerListKind } from "../../api/lcu";
import { autoTagLabel, isGoodAutoTag } from "../../api/lcu/savedPlayers";
import { useToast } from "../../composables/useToast";
import { usePlayerSearch } from "../../composables/usePlayerSearch";
import {
  computeAvgCs,
  computeAvgDamageRatio,
  computeAvgVision,
  formatStreakBadge,
  getKdaClass,
  getWinRateClass,
} from "../../composables/playerCardStats";
import {
  formatTierShort,
  getTierColor,
  getTierMedal,
} from "../../utils/rankedDisplay";
import LcuImage from "../LcuImage.vue";
import PlayerMatchList from "./PlayerMatchList.vue";

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
const lcuStore = useLcuStore();
const { handleCareerClick } = usePlayerSearch();
const { showToast } = useToast();
const dialog = useDialog();
const openOpgg = inject<(championId?: number) => void>("openOpgg");

function onGuideClick(e: MouseEvent) {
  e.stopPropagation();
  if (resolvedChampId.value > 0) {
    openOpgg?.(resolvedChampId.value);
  } else {
    openOpgg?.();
  }
}

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

const formatRank = (entry: typeof soloRank.value) =>
  formatTierShort(entry, t("gameInfo.unranked"));

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

const matches = computed(() => props.playerData?.matches ?? []);

const avgCs = computed(() => computeAvgCs(matches.value));
const avgVision = computed(() => computeAvgVision(matches.value));
const avgDamageRatio = computed(() => computeAvgDamageRatio(matches.value));

const streakBadge = computed(() => formatStreakBadge(props.playerData?.streak));

const cardTags = computed(() => {
  const tags: { text: string; cls: string; bg?: string; color?: string }[] = [];
  const data = props.playerData;
  if (!data) return tags;

  if (props.selfPuuid && data.info?.puuid && data.info.puuid === props.selfPuuid) {
    tags.push({ text: t("gameInfo.tagSelf"), cls: "tag-self" });
  }
  const puuid = data.info?.puuid;
  if (puuid && props.savedMap?.[puuid]) {
    const marker = props.savedMap[puuid];
    if (marker.listKind === "black") {
      tags.push({
        text: marker.listReason ? `拉黑:${marker.listReason}` : "拉黑",
        cls: "tag-black",
      });
    } else if (marker.listKind === "white") {
      tags.push({ text: "白名单", cls: "tag-white" });
    }
    if (marker.tag) {
      tags.push({ text: t("gameInfo.tagMarked"), cls: "tag-marked" });
    }
    // 自动标签（key 或旧中文串，统一走 helper 归一化）
    if (marker.autoTag) {
      tags.push({
        text: autoTagLabel(marker.autoTag),
        cls: isGoodAutoTag(marker.autoTag) ? "tag-auto-good" : "tag-auto-bad",
      });
    }
    // 上次关系
    if (marker.lastRelation === "ally") {
      tags.push({ text: "曾同队", cls: "tag-rel-ally" });
    } else if (marker.lastRelation === "enemy") {
      tags.push({ text: "曾对手", cls: "tag-rel-enemy" });
    }
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

const isMatchHidden = computed(() => props.playerData?.matchHistoryHidden === true);
const isLoading = computed(() => props.playerData?.loading === true);
/** 对局中 LCU match-history 常不可用：区分「暂无数据」与「暂时拉不到」 */
const isHistoryUnreachable = computed(() => {
  if (isLoading.value || isMatchHidden.value) return false;
  if (matches.value.length > 0) return false;
  return (
    lcuStore.gamePhase === "InProgress" || lcuStore.gamePhase === "GameStart"
  );
});

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

async function setList(kind: "black" | "white" | "") {
  const p = summonerInfo.value?.puuid;
  const selfPuuid = props.selfPuuid;
  if (!p || !selfPuuid) return;

  const apply = async (reason: string | null) => {
    try {
      await setPlayerListKind(
        selfPuuid,
        p,
        kind,
        reason,
        displayName.value || null,
      );
      showToast(
        kind === "black"
          ? t("gameInfo.blacklistSuccess")
          : kind === "white"
            ? t("gameInfo.whitelistSuccess")
            : t("gameInfo.listRemoveSuccess"),
        "success",
      );
    } catch (e) {
      console.error("[PlayerInfoCard] 名单操作失败:", e);
      showToast(t("gameInfo.listActionFailed"), "error");
    }
  };

  if (kind !== "black") {
    await apply(null);
    return;
  }

  const reasonInput = ref("");
  dialog.create({
    title: t("gameInfo.blacklistTitle"),
    content: () =>
      h(NInput, {
        value: reasonInput.value,
        placeholder: t("gameInfo.blacklistReasonPlaceholder"),
        onUpdateValue: (v: string) => {
          reasonInput.value = v;
        },
      }),
    positiveText: t("tools.confirm"),
    negativeText: t("tools.cancel"),
    onPositiveClick: () => apply(reasonInput.value.trim() || null),
  });
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
          <template v-if="selfPuuid && summonerInfo?.puuid && summonerInfo.puuid !== selfPuuid">
            <button
              class="list-btn black"
              title="拉黑"
              @click.stop="setList('black')"
            >
              黑
            </button>
            <button
              class="list-btn white"
              title="加白"
              @click.stop="setList('white')"
            >
              白
            </button>
            <button
              v-if="savedMap?.[summonerInfo.puuid]?.listKind"
              class="list-btn clear"
              title="移出名单"
              @click.stop="setList('')"
            >
              ×
            </button>
          </template>
        </div>

        <div class="rank-row">
          <div class="rank-item" :style="{ color: getTierColor(soloRank?.tier) }">
            <img v-if="getTierMedal(soloRank?.tier)" :src="getTierMedal(soloRank?.tier)!" class="rank-medal" alt="" />
            <span class="rank-label">{{ $t("gameInfo.soloRank") }}:</span>
            <span class="rank-val">{{ formatRank(soloRank) }}</span>
          </div>
          <div class="rank-item" :style="{ color: getTierColor(flexRank?.tier) }">
            <img v-if="getTierMedal(flexRank?.tier)" :src="getTierMedal(flexRank?.tier)!" class="rank-medal" alt="" />
            <span class="rank-label">{{ $t("gameInfo.flexRank") }}:</span>
            <span class="rank-val">{{ formatRank(flexRank) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 擅长英雄 / 攻略 -->
    <div class="mastery-row">
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
      <button
        v-if="openOpgg"
        class="guide-btn"
        :title="resolvedChampId > 0 ? '查看该英雄 OP.GG 攻略' : '打开 OP.GG'"
        @click="onGuideClick"
      >
        攻略
      </button>
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
        <span v-if="avgVision !== undefined" class="si">
          <span class="si-l">视野:</span>
          <span class="si-v">{{ avgVision }}</span>
        </span>
        <span v-if="avgDamageRatio !== undefined" class="si">
          <span class="si-l">伤转:</span>
          <span class="si-v" :class="avgDamageRatio >= 100 ? 'stat-win' : avgDamageRatio < 80 ? 'stat-loss' : ''">
            {{ avgDamageRatio }}%
          </span>
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
    <PlayerMatchList
      :matches="matches"
      :is-loading="isLoading"
      :is-match-hidden="isMatchHidden"
      :is-history-unreachable="isHistoryUnreachable"
    />
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
.guide-btn {
  margin-left: auto;
  flex-shrink: 0;
  height: 18px;
  padding: 0 7px;
  border: none;
  border-radius: 4px;
  background: var(--primary-color-alpha-15, rgba(0, 210, 196, 0.15));
  color: var(--primary-color, #00d2c4);
  font-size: 10px;
  font-weight: 700;
  cursor: pointer;
  line-height: 18px;
  transition: background 0.12s ease;
}
.guide-btn:hover {
  background: var(--primary-color-alpha-30, rgba(0, 210, 196, 0.3));
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
.tag-auto-good {
  background: linear-gradient(135deg, rgba(245, 158, 11, 0.25), rgba(234, 88, 12, 0.2));
  color: #b45309;
  font-weight: 800;
  box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.35);
}
.tag-auto-bad {
  background: linear-gradient(135deg, rgba(220, 38, 38, 0.18), rgba(190, 18, 60, 0.15));
  color: #b91c1c;
  font-weight: 800;
  box-shadow: 0 0 0 1px rgba(220, 38, 38, 0.3);
}
.tag-rel-ally {
  background: rgba(59, 130, 246, 0.12);
  color: var(--tier-blue, #3b82f6);
}
.tag-rel-enemy {
  background: rgba(244, 63, 94, 0.1);
  color: #e11d48;
}
.tag-black {
  background: rgba(127, 29, 29, 0.2);
  color: #fecaca;
  font-weight: 800;
  box-shadow: 0 0 0 1px rgba(220, 38, 38, 0.45);
}
.tag-white {
  background: rgba(16, 185, 129, 0.15);
  color: #047857;
  font-weight: 700;
}
.list-btn {
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 800;
  cursor: pointer;
  padding: 0;
  line-height: 18px;
  margin-left: 2px;
}
.list-btn.black {
  background: rgba(220, 38, 38, 0.15);
  color: #dc2626;
}
.list-btn.white {
  background: rgba(16, 185, 129, 0.15);
  color: #059669;
}
.list-btn.clear {
  background: rgba(0, 0, 0, 0.08);
  color: var(--text-dimmed);
}
.list-btn:hover {
  filter: brightness(1.15);
}
</style>
