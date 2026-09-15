<script setup lang="ts">
import type { RankDisplaySource } from "../../types/lcu";
import { formatRankDisplay } from "../../utils/queueMeta";

defineProps<{
  soloQueue: RankDisplaySource | null;
  flexQueue: RankDisplaySource | null;
}>();

function formatRank(queue: RankDisplaySource | null) {
  if (!queue) return "--";
  const raw = queue.rank && queue.rank !== "NA" ? queue.rank : queue.division;
  return formatRankDisplay(queue.tier, raw);
}

function formatHighestRank(queue: RankDisplaySource | null) {
  if (!queue) return "--";
  return formatRankDisplay(queue.highestTier, queue.highestRank);
}

function formatPrevSeasonRank(queue: RankDisplaySource | null) {
  if (!queue) return "--";
  return formatRankDisplay(queue.previousSeasonEndTier, queue.previousSeasonEndRank);
}
</script>

<template>
    <!-- 排位段位信息表 -->
    <div class="rank-table-wrapper">
      <table class="rank-table">
        <thead>
          <tr>
            <th>{{ $t("career.type") }}</th>
            <th>{{ $t("career.totalGames") }}</th>
            <th>{{ $t("career.winRate") }}</th>
            <th>{{ $t("career.winsLabel") }}</th>
            <th>{{ $t("career.lossesLabel") }}</th>
            <th>{{ $t("career.tier") }}</th>
            <th>{{ $t("career.lp") }}</th>
            <th>{{ $t("career.highest") }}</th>
            <th>{{ $t("career.prevSeason") }}</th>
          </tr>
        </thead>
        <tbody>
          <!-- 单双排 -->
          <tr>
            <td class="type-name">{{ $t("gameModes.420") }}</td>
            <td>{{ soloQueue ? soloQueue.wins + soloQueue.losses : 0 }}</td>
            <td>
              {{
                soloQueue && soloQueue.wins + soloQueue.losses > 0
                  ? (
                      (soloQueue.wins / (soloQueue.wins + soloQueue.losses)) *
                      100
                    ).toFixed(0) + "%"
                  : "--"
              }}
            </td>
            <td>{{ soloQueue ? soloQueue.wins : 0 }}</td>
            <td>{{ soloQueue ? soloQueue.losses : 0 }}</td>
            <td class="rank-name">
              {{ soloQueue ? formatRank(soloQueue) : "--" }}
            </td>
            <td>{{ soloQueue ? soloQueue.leaguePoints : 0 }}</td>
            <td>{{ soloQueue ? formatHighestRank(soloQueue) : "--" }}</td>
            <td>{{ soloQueue ? formatPrevSeasonRank(soloQueue) : "--" }}</td>
          </tr>
          <!-- 灵活排位 -->
          <tr>
            <td class="type-name">{{ $t("gameModes.440") }}</td>
            <td>{{ flexQueue ? flexQueue.wins + flexQueue.losses : 0 }}</td>
            <td>
              {{
                flexQueue && flexQueue.wins + flexQueue.losses > 0
                  ? (
                      (flexQueue.wins / (flexQueue.wins + flexQueue.losses)) *
                      100
                    ).toFixed(0) + "%"
                  : "--"
              }}
            </td>
            <td>{{ flexQueue ? flexQueue.wins : 0 }}</td>
            <td>{{ flexQueue ? flexQueue.losses : 0 }}</td>
            <td class="rank-name">
              {{ flexQueue ? formatRank(flexQueue) : "--" }}
            </td>
            <td>{{ flexQueue ? flexQueue.leaguePoints : 0 }}</td>
            <td>{{ flexQueue ? formatHighestRank(flexQueue) : "--" }}</td>
            <td>{{ flexQueue ? formatPrevSeasonRank(flexQueue) : "--" }}</td>
          </tr>
        </tbody>
      </table>
    </div>

</template>

<style scoped>
/* 排位数据表 */
.rank-table-wrapper {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.rank-table-wrapper:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.rank-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.rank-table th,
.rank-table td {
  padding: 6px 16px;
  font-size: 0.82rem;
  border-bottom: 1px solid var(--border-color);
}

.rank-table th {
  background-color: rgba(0, 0, 0, 0.01);
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1.5px solid var(--border-color);
}

.rank-table tr:last-child td {
  border-bottom: none;
}

.type-name {
  font-weight: 600;
  color: var(--text-color);
}

.rank-name {
  font-weight: bold;
  color: var(--primary-color);
}


</style>
