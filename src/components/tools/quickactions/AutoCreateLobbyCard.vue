<script setup lang="ts">
import { useAutoSaveConfig } from "../../../composables/useAutoSaveConfig";
import { useI18n } from "vue-i18n";
import { NSwitch, NSelect, NCollapse, NCollapseItem } from "naive-ui";
import { QUEUE_NAME_MAP } from "../../../utils/queueMeta";

const { config, triggerAutoSave } = useAutoSaveConfig();
const { t } = useI18n();

/** 建厅可选队列 ID 列表；展示名统一取自 QUEUE_NAME_MAP */
const LOBBY_QUEUE_IDS = [
  2400, 2450, 450, 430, 420, 440, 900, 1020, 1300, 1700, 4300,
] as const;

const GAME_MODES: { id: number; name: string }[] = LOBBY_QUEUE_IDS.map(
  (id) => ({ id, name: QUEUE_NAME_MAP[id] ?? String(id) }),
);
</script>

<template>
  <!-- 自动创建大厅 -->
  <n-collapse
    v-if="config"
    arrow-placement="right"
    class="collapse-card"
  >
    <n-collapse-item name="createlobby">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left">
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
                <path
                  d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
                ></path>
                <polyline points="9 22 9 12 15 12 15 22"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">
                {{ t("tools.autoCreateLobby.title") }}
              </h3>
              <span class="card-desc">{{
                t("tools.autoCreateLobby.desc")
              }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">
              {{
                config?.Functions.EnableAutoCreateLobby
                  ? t("tools.autoCreateLobby.enabled", {
                      mode: config?.Functions.DefaultGameMode
                        ? t(
                            "gameModes." + config.Functions.DefaultGameMode,
                          )
                        : t("tools.autoCreateLobby.unknownMode"),
                    })
                  : t("tools.autoCreateLobby.disabled")
              }}
            </span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{
          t("tools.autoCreateLobby.label")
        }}</span>
        <n-switch
          :value="config.Functions.EnableAutoCreateLobby"
          @update:value="
            (v: boolean) => {
              if (config) config.Functions.EnableAutoCreateLobby = v;
              triggerAutoSave();
            }
          "
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{
          t("tools.autoCreateLobby.defaultMode")
        }}</span>
        <n-select
          :value="config.Functions.DefaultGameMode"
          :options="
            GAME_MODES.map((m) => ({
              label: t('gameModes.' + m.id),
              value: m.id,
            }))
          "
          :disabled="!config.Functions.EnableAutoCreateLobby"
          @update:value="
            (v: number) => {
              if (config) config.Functions.DefaultGameMode = v;
              triggerAutoSave();
            }
          "
          style="width: 140px"
          size="small"
        />
      </div>
    </n-collapse-item>
  </n-collapse>
</template>

<style scoped>
.collapse-header-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.collapse-left {
  display: flex;
  align-items: center;
  flex: 1;
  gap: 14px;
}

.icon-container {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.header-icon {
  width: 18px;
  height: 18px;
  stroke-width: 2px;
}

.title-container {
  display: flex;
  flex-direction: column;
}

.card-title {
  font-size: 0.88rem;
  font-weight: bold;
  color: var(--text-color);
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 4px;
  line-height: 1.4;
}

.collapse-right-status {
  margin-left: auto;
  display: flex;
  align-items: center;
}

.status-preview {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin-right: 10px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px dashed var(--border-color);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}
</style>
