import { invoke } from "@tauri-apps/api/core";

// ─── 应用配置（读写）───
// 字段名使用 PascalCase，与 Rust serde(rename_all = "PascalCase") 一致

export interface GeneralConfig {
  LolPath: string[];
  /** WeGame 客户端安装路径（对应 LolPath 中的 "WeGame" 标记条目） */
  WegamePath: string | null;
  EnableStartLolWithApp: boolean;
  EnableCloseToTray: boolean | null;
  EnableGameStartMinimize: boolean;
  EnableCheckUpdate: boolean;
  LogLevel: number;
  EnableHttpProxy: boolean;
  HttpProxyAddr: string;
  EnableSignalrHub: boolean;
  SignalrServerUrl: string;
  SignalrUserId: string;
  UploadApiUrl: string;
}

export interface PersonalizationConfig {
  MicaEnabled: boolean;
  DpiScale: string;
  Language: string;
  ThemeMode: string;
  WinCardColor: string;
  LoseCardColor: string;
  RemakeCardColor: string;
  LightDeathsNumberColor: string;
  DarkDeathsNumberColor: string;
  ThemeColor: string;
}

export interface FunctionsConfig {
  CareerGamesNumber: number;
  ApiConcurrencyNumber: number;
  GameInfoFilter: boolean;
  ShowTierInGameInfo: boolean;
  AutoShowOpgg: boolean;
  EnableOpggOnTop: boolean;
  EnableAutoAcceptMatching: boolean;
  EnableAutoReconnect: boolean;
  EnableAutoCreateLobby: boolean;
  DefaultGameMode: number;
  AutoAcceptMatchingDelay: number;
  EnableAutoHoverChampion: boolean;
  AutoSelectConfirmOnTimeout: boolean;
  EnableRandomSkin: boolean;
  EnableAutoSelectChampion: boolean;
  AutoSelectChampion: number[];
  AutoSelectChampionTop: number[];
  AutoSelectChampionJug: number[];
  AutoSelectChampionMid: number[];
  AutoSelectChampionBot: number[];
  AutoSelectChampionSup: number[];
  EnableAutoBanChampion: boolean;
  AutoBanChampion: number[];
  AutoBanChampionTop: number[];
  AutoBanChampionJug: number[];
  AutoBanChampionMid: number[];
  AutoBanChampionBot: number[];
  AutoBanChampionSup: number[];
  AutoBanDelay: number;
  PretendBan: boolean;
  AutoAcceptCeilSwap: boolean;
  AutoAcceptChampTrade: boolean;
  EnableAutoSetSpells: boolean;
  AutoSetSummonerSpell: number[];
  AutoSetSummonerSpellTop: number[];
  AutoSetSummonerSpellJug: number[];
  AutoSetSummonerSpellMid: number[];
  AutoSetSummonerSpellBot: number[];
  AutoSetSummonerSpellSup: number[];
  EnableReserveGameinfo: boolean;
  LcuRealtimeEnabled: boolean;
  LcuUserId: string;
  UploadEnabled: boolean;
  HideTft: boolean;
  HideSavedPlayers: boolean;
  EnableBenchOverlay: boolean;
  EnableScreenshotOnMultikill: boolean;
  ScreenshotOnMultikillLevels: number[];
  ScreenshotSavePath: string;
  EnableAutoHandleInvite: boolean;
  EnableAutoHonor: boolean;
  EnableAutoPlayAgain: boolean;
  EnableAutoAramTeamSide: boolean;
  AramTeamSideVisibleToTeam: boolean;
  EnableAutoTagReminder: boolean;
  /** 对局结束自动给极端表现玩家打标 */
  EnableAutoPlayerTag: boolean;
  /** 0 严格 / 1 标准 / 2 宽松 */
  AutoTagSensitivity: number;
  /** 选人发现黑名单队友时提示秒退 */
  EnableDodgeReminder: boolean;
}

export interface OtherConfig {
  LastNoticeSha: string;
  SearchHistory: string;
}

export interface AppConfig {
  Version?: number;
  General: GeneralConfig;
  Personalization: PersonalizationConfig;
  Functions: FunctionsConfig;
  Other: OtherConfig;
}

/**
 * 获取完整应用配置。
 * 启动早期窗口 JS 可能抢在 Rust 侧 app.manage(AppState) 之前调用，
 * 此时后端会报 "state not managed"，这里做短暂自愈重试，其余错误直接抛出。
 */
export const fetchConfig = () => {
  const attempt = (left: number): Promise<AppConfig> =>
    invoke<AppConfig>("get_config").catch((err) => {
      const errStr = String(err || "");
      const isStateNotReady =
        errStr.includes("state not managed") || errStr.includes("must call .manage()");
      if (isStateNotReady && left > 0) {
        const delay = 200 * (5 - left + 1);
        return new Promise<void>((resolve) => setTimeout(resolve, delay)).then(() =>
          attempt(left - 1),
        );
      }
      throw err;
    });
  return attempt(5);
};

/** 更新完整应用配置 */
export const updateConfig = (config: AppConfig) =>
  invoke<void>("update_config", { newConfig: config });
