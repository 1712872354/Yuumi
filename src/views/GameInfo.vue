<script setup lang="ts">
import { ref, computed, inject, type Ref } from "vue";
import { onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../store/lcuStore";
import type { AppConfig, SavedPlayerMarker } from "../api/lcu";
import { querySavedPlayersMap } from "../api/lcu";
import {
  PREMADE_COLORS,
  getChampionIcon,
  type PlayerData,
  type PremadePlayerLike,
} from "../types/gameInfo";
import { usePremadeGroup } from "../composables/usePremadeGroup";
import { useGamePlayerData, isIdentityCompatible } from "../composables/useGamePlayerData";
import LcuOfflineState from "../components/LcuOfflineState.vue";
import PlayerInfoCard from "../components/gameinfo/PlayerInfoCard.vue";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const { t } = useI18n();
const activeTab = ref<"my" | "their">("my");

const appConfig =
  inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);

const premadeColorsMy = ref<Record<number, number>>({});
const premadeColorsTheir = ref<Record<number, number>>({});

const {
  playerData,
  sessionAllyTeam,
  sessionEnemyTeam,
  isTftMode,
  myTeam,
  theirTeam,
  shouldShowContent,
  currentSummonerPuuid,
} = useGamePlayerData(
  appConfig,
  premadeColorsMy,
  premadeColorsTheir,
  activeTab,
);

const { getPremadeIdx, myPremadeGroups, theirPremadeGroups } = usePremadeGroup(
  myTeam,
  theirTeam,
  sessionAllyTeam,
  sessionEnemyTeam,
  playerData,
  premadeColorsMy,
  premadeColorsTheir,
);

function isSameIdentity(p: PremadePlayerLike, d: PlayerData | undefined): boolean {
  if (!d) return false;
  if (!d.info) return true;
  const dSidReal = d.info.summonerId && d.info.summonerId !== p.cellId ? d.info.summonerId : 0;
  if (!p.puuid && !p.summonerId && (d.info.puuid || dSidReal)) return false;
  return isIdentityCompatible(d, { puuid: p.puuid, summonerId: p.summonerId, cellId: p.cellId });
}

function getPlayerData(p: PremadePlayerLike, idx: number, side: "ally" | "enemy") {
  if (p.puuid) {
    const byPuuid = playerData.value[p.puuid];
    if (byPuuid && isSameIdentity(p, byPuuid)) return byPuuid;
  }
  if (p.summonerId) {
    const bySid = playerData.value[p.summonerId];
    if (bySid && isSameIdentity(p, bySid)) return bySid;
  }
  if (p.cellId !== undefined) {
    const byCell = playerData.value[p.cellId];
    if (byCell && isSameIdentity(p, byCell)) return byCell;
  }
  const offset = side === "enemy" ? 5 : 0;
  const bySlot = playerData.value[offset + idx];
  if (bySlot && isSameIdentity(p, bySlot)) return bySlot;

  for (const key of Object.keys(playerData.value)) {
    const d = playerData.value[key];
    if (!d?.info) continue;
    if (p.puuid && d.info.puuid && d.info.puuid === p.puuid) return d;
    if (p.summonerId && d.info.summonerId && d.info.summonerId === p.summonerId) return d;
    const pName = p.displayName || p.gameName || p.summonerName;
    const dName = d.info.displayName || d.info.gameName;
    if (pName && dName && pName === dName) return d;
  }
  return undefined;
}

const savedPlayerMap = ref<Record<string, SavedPlayerMarker>>({});

const isTheirTeamRevealed = computed(() => {
  if (theirTeam.value.length === 0) return false;
  if (store.gamePhase === "ChampSelect") {
    return theirTeam.value.some(
      (p: PremadePlayerLike) =>
        Boolean(
          p.summonerId ||
          p.puuid ||
          p.championId ||
          p.botChampionId ||
          p.bot ||
          p.isBot ||
          p.displayName ||
          p.summonerName,
        ),
    );
  }
  return true;
});

let savedPlayerMapInflight: Promise<void> | null = null;

async function loadSavedPlayerMap() {
  const puuid = currentSummonerPuuid.value;
  if (!puuid || !store.isConnected) return;
  if (savedPlayerMapInflight) return;
  savedPlayerMapInflight = (async () => {
    try {
      savedPlayerMap.value = await querySavedPlayersMap(puuid);
    } catch (e) {
      console.error("[GameInfo] 保存玩家映射加载失败:", e);
    } finally {
      savedPlayerMapInflight = null;
    }
  })();
}

watch(() => store.gamePhase, (phase) => {
  if (phase === "ChampSelect") loadSavedPlayerMap();
});
watch(() => store.isConnected, () => loadSavedPlayerMap());
watch(currentSummonerPuuid, () => loadSavedPlayerMap());
onMounted(() => {
  loadSavedPlayerMap();
});

function teamWinRate(players: PremadePlayerLike[], side: "ally" | "enemy"): string {
  let wins = 0;
  let total = 0;
  players.forEach((p, i) => {
    const d = getPlayerData(p, i, side);
    if (!d) return;
    wins += d.winCount ?? 0;
    total += (d.winCount ?? 0) + (d.lossesCount ?? 0);
  });
  if (total === 0) return "—";
  return `${Math.round((wins / total) * 100)}%`;
}

function premadeChipStyle(group: { colorIdx: number }) {
  const c = PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length];
  return { borderColor: c.border, backgroundColor: c.bg };
}

function premadeDotStyle(group: { colorIdx: number }) {
  const c = PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length];
  return { background: c.dot };
}

function getPremadeSize(p: PremadePlayerLike, team: PremadePlayerLike[], side: "my" | "their"): number | undefined {
  const idx = getPremadeIdx(p, side);
  if (idx === undefined || idx < 0) return undefined;
  let n = 0;
  for (const m of team) {
    if (getPremadeIdx(m, side) === idx) n++;
  }
  return n >= 2 ? n : undefined;
}
</script>

<template>
  <div class="game-info">
    <LcuOfflineState v-if="!store.isConnected" />

    <div v-else-if="isTftMode" class="tip-container">
      <div class="offline-logo">♟️</div>
      <p class="tip">云顶之弈对局中，对局信息页面不显示数据</p>
    </div>

    <div v-else-if="!shouldShowContent" class="tip-container">
      <div class="offline-logo">⏳</div>
      <p class="tip">{{ t("gameInfo.awaitingLoad") }}</p>
    </div>

    <div v-else class="teams-board">
      <!-- ─── 蓝方 ─── -->
      <section class="team-section ally-section">
        <header class="team-header">
          <span class="team-dot ally-dot"></span>
          <span class="team-title">{{ t("gameInfo.myTeam", { count: myTeam.length }) }}</span>
          <div v-if="myPremadeGroups.length" class="premade-chips">
            <div
              v-for="group in myPremadeGroups"
              :key="group.colorIdx"
              class="premade-chip"
              :style="premadeChipStyle(group)"
              :title="t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
            >
              <span class="chip-dot" :style="premadeDotStyle(group)"></span>
              <div class="chip-avatars">
                <template v-for="m in group.members" :key="m.summonerId">
                  <LcuImage
                    v-if="m.championId > 0"
                    :src="getChampionIcon(m.championId)"
                    class="chip-avatar"
                    :title="m.displayName"
                  />
                  <div v-else class="chip-avatar chip-avatar-empty" :title="m.displayName">
                    {{ m.displayName ? m.displayName.slice(0, 1) : "?" }}
                  </div>
                </template>
              </div>
            </div>
          </div>
          <span class="team-wr">胜率: {{ teamWinRate(myTeam, "ally") }}</span>
        </header>

        <div class="team-grid">
          <PlayerInfoCard
            v-for="(p, i) in myTeam"
            :key="p.cellId ?? p.puuid ?? p.summonerId ?? i"
            :player="p"
            :player-data="getPlayerData(p, i, 'ally')"
            side="ally"
            :premade-idx="getPremadeIdx(p, 'my')"
            :premade-size="getPremadeSize(p, myTeam, 'my')"
            :saved-map="savedPlayerMap"
            :self-puuid="currentSummonerPuuid"
            :index="i"
          />
        </div>
      </section>

      <!-- ─── 红方 ─── -->
      <section v-if="isTheirTeamRevealed" class="team-section enemy-section">
        <header class="team-header">
          <span class="team-dot enemy-dot"></span>
          <span class="team-title">{{ t("gameInfo.theirTeam", { count: theirTeam.length }) }}</span>
          <div v-if="theirPremadeGroups.length" class="premade-chips">
            <div
              v-for="group in theirPremadeGroups"
              :key="group.colorIdx"
              class="premade-chip"
              :style="premadeChipStyle(group)"
              :title="t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
            >
              <span class="chip-dot" :style="premadeDotStyle(group)"></span>
              <div class="chip-avatars">
                <template v-for="m in group.members" :key="m.summonerId">
                  <LcuImage
                    v-if="m.championId > 0"
                    :src="getChampionIcon(m.championId)"
                    class="chip-avatar"
                    :title="m.displayName"
                  />
                  <div v-else class="chip-avatar chip-avatar-empty" :title="m.displayName">
                    {{ m.displayName ? m.displayName.slice(0, 1) : "?" }}
                  </div>
                </template>
              </div>
            </div>
          </div>
          <span class="team-wr">胜率: {{ teamWinRate(theirTeam, "enemy") }}</span>
        </header>

        <div class="team-grid">
          <PlayerInfoCard
            v-for="(p, i) in theirTeam"
            :key="p.cellId ?? p.puuid ?? p.summonerId ?? i"
            :player="p"
            :player-data="getPlayerData(p, i, 'enemy')"
            side="enemy"
            :premade-idx="getPremadeIdx(p, 'their')"
            :premade-size="getPremadeSize(p, theirTeam, 'their')"
            :saved-map="savedPlayerMap"
            :self-puuid="currentSummonerPuuid"
            :index="i"
          />
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.game-info {
  padding: 10px 14px 10px 8px;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: auto;
  min-height: 0;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}
.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}
.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* ─── 双队看板：按内容撑高，整页滚动，避免压缩叠层 ─── */
.teams-board {
  display: flex;
  flex-direction: column;
  gap: 16px;
  /* 不 flex:1，不 min-height:0，让高度由内容决定 */
}

.team-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.team-header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  flex-wrap: wrap;
}

.team-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.25);
  flex-shrink: 0;
}
.ally-dot {
  background: #3b82f6;
}
.enemy-dot {
  background: #f43f5e;
}

.team-title {
  font-size: 14px;
  font-weight: 800;
  color: var(--text-color, #1f2937);
  letter-spacing: 0.02em;
}

.team-wr {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-dimmed, #6b7280);
  margin-left: auto;
}

.premade-chips {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin-left: 8px;
}

.premade-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 2px 6px;
  border-radius: 6px;
  border: 1px solid transparent;
  font-size: 11px;
}

.chip-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.chip-avatars {
  display: flex;
  gap: 2px;
}

.chip-avatar {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  object-fit: cover;
}
.chip-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.15);
  color: #fff;
  font-size: 10px;
  font-weight: 700;
}

/* ─── 卡片网格：固定最小行高，列数随宽度自适应，不叠层 ─── */
.team-grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 10px;
  /* 行高有下限，卡片内部战绩区自己滚动 */
  grid-auto-rows: minmax(340px, 380px);
  align-items: stretch;
}

@media (max-width: 1500px) {
  .team-grid {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}
@media (max-width: 1200px) {
  .team-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
@media (max-width: 900px) {
  .team-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 600px) {
  .team-grid {
    grid-template-columns: 1fr;
  }
}
</style>
