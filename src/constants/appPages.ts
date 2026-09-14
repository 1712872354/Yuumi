import type { Component } from "vue";
import Home from "../views/Home.vue";
import Career from "../views/Career.vue";
import Search from "../views/Search.vue";
import GameInfo from "../views/GameInfo.vue";
import TFT from "../views/TFT.vue";
import SavedPlayers from "../views/SavedPlayers.vue";
import { defineAsyncComponent } from "vue";

const Settings = defineAsyncComponent(() => import("../views/Settings.vue"));
const Tools = defineAsyncComponent(() => import("../views/Tools.vue"));

/** 业务主页面（v-show 保活 + hasVisited 延迟挂载） */
export interface KeepAlivePageDef {
  key: string;
  component: Component;
  /** 返回 true 时按配置隐藏，不挂载 */
  hiddenWhen?: boolean;
}

export function buildKeepAlivePages(opts: {
  hideTft: boolean;
  hideSavedPlayers: boolean;
}): KeepAlivePageDef[] {
  return [
    { key: "gameinfo", component: GameInfo },
    { key: "search", component: Search },
    { key: "career", component: Career },
    { key: "tft", component: TFT, hiddenWhen: opts.hideTft },
    { key: "settings", component: Settings },
    { key: "tools", component: Tools },
    { key: "savedplayers", component: SavedPlayers, hiddenWhen: opts.hideSavedPlayers },
  ];
}

export { Home };
