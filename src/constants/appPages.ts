import { defineAsyncComponent, h, type Component } from "vue";
import Home from "../views/Home.vue";

// 轻量加载占位：本地磁盘加载极快，仅防首次进入时的白屏闪烁
const AsyncLoading = {
  render: () =>
    h(
      "div",
      {
        class: "async-page-loading",
        style:
          "display:flex;align-items:center;justify-content:center;height:100%;min-height:200px;color:var(--text-muted);font-size:0.9rem;",
      },
      "加载中…",
    ),
};

// 加载失败兜底，避免整页空白
const AsyncError = {
  render: () =>
    h(
      "div",
      {
        class: "async-page-error",
        style:
          "display:flex;align-items:center;justify-content:center;height:100%;min-height:200px;color:var(--win-color);font-size:0.9rem;",
      },
      "页面加载失败，请重试",
    ),
};

/** 视图级懒加载：首次访问时才拉取对应 chunk，缩小首屏主包 */
const Career = defineAsyncComponent({
  loader: () => import("../views/Career.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});
const Search = defineAsyncComponent({
  loader: () => import("../views/Search.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});
const GameInfo = defineAsyncComponent({
  loader: () => import("../views/GameInfo.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});
const TFT = defineAsyncComponent({
  loader: () => import("../views/TFT.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});
const SavedPlayers = defineAsyncComponent({
  loader: () => import("../views/SavedPlayers.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});
const Settings = defineAsyncComponent({
  loader: () => import("../views/Settings.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});
const Tools = defineAsyncComponent({
  loader: () => import("../views/Tools.vue"),
  loadingComponent: AsyncLoading,
  errorComponent: AsyncError,
  delay: 0,
  timeout: 10000,
});

/** 业务主页（v-show 保活 + hasVisited 延迟挂载，见 constants/appPages.ts） */
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
