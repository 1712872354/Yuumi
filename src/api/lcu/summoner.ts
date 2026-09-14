import { invoke } from "@tauri-apps/api/core";
import { lcuRequest } from "./core";

export interface SummonerDisplay {
  accountId: number;
  displayName: string;
  gameName: string;
  tagLine: string;
  percentCompleteForNextLevel: number;
  profileIconId: number;
  puuid: string;
  summonerId: number;
  summonerLevel: number;
  xpSinceLastLevel: number;
  xpUntilNextLevel: number;
  profileIconUrl: string;
}

/** 获取当前召唤师信息（Rust 解析层清洗后，404 时自动重试） */
let summonerInflight: Promise<SummonerDisplay> | null = null;

export function fetchCurrentSummoner(
  maxRetries = 15,
): Promise<SummonerDisplay> {
  if (!summonerInflight) {
    summonerInflight = doFetchCurrentSummoner(maxRetries).finally(() => {
      summonerInflight = null;
    });
  }
  return summonerInflight;
}

async function doFetchCurrentSummoner(
  maxRetries: number,
): Promise<SummonerDisplay> {
  for (let i = 0; i <= maxRetries; i++) {
    try {
      return await invoke<SummonerDisplay>("get_current_summoner");
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("404") && i < maxRetries) {
        await new Promise((r) => setTimeout(r, 2000));
        continue;
      }
      if (msg.includes("404")) {
        throw new Error("未登录：请在英雄联盟客户端中登录您的账号");
      }
      throw e;
    }
  }
  throw new Error("获取召唤师信息失败");
}

/** 根据 puuid 获取召唤师信息（通过 LCU v2 接口） */
export async function fetchSummonerByPuuid(
  puuid: string,
): Promise<SummonerDisplay | null> {
  const resp = await lcuRequest<Partial<SummonerDisplay>>(
    "GET",
    `/lol-summoner/v2/summoners/puuid/${puuid}`,
  );
  if (!resp.success || !resp.data) return null;
  const data = resp.data;
  return {
    accountId: data.accountId ?? 0,
    displayName: data.displayName ?? "",
    gameName: data.gameName ?? "",
    tagLine: data.tagLine ?? "",
    percentCompleteForNextLevel: data.percentCompleteForNextLevel ?? 0,
    profileIconId: data.profileIconId ?? 29,
    puuid: data.puuid ?? puuid,
    summonerId: data.summonerId ?? 0,
    summonerLevel: data.summonerLevel ?? 0,
    xpSinceLastLevel: data.xpSinceLastLevel ?? 0,
    xpUntilNextLevel: data.xpUntilNextLevel ?? 0,
    profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${data.profileIconId ?? 29}.jpg`,
  };
}
