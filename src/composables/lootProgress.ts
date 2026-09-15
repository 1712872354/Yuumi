/** 战利品批量操作进度事件监听助手 */
import { listen } from "@tauri-apps/api/event";
import type { ActionProgressEvent, LootProgressEvent } from "../api/loot";

/** 将 ActionProgressEvent 转换为 LootProgressEvent，用于分解/升级/重随进度展示 */
export function toLootProgress(
  evt: ActionProgressEvent,
  displayName: string,
  isReroll = false,
): LootProgressEvent {
  if (evt.success) {
    return {
      current: evt.current,
      total: evt.total,
      success: true,
      rewardName: isReroll
        ? `合成成功！获得: ${evt.rewardDesc}`
        : `${displayName}: ${evt.rewardDesc}`,
      errorMsg: null,
    };
  }
  return {
    current: evt.current,
    total: evt.total,
    success: false,
    rewardName: "",
    errorMsg: isReroll
      ? (evt.errorMsg ?? "未知错误")
      : `${displayName}: ${evt.errorMsg}`,
  };
}

export type ProgressUnlisten = () => void;

/** 注册进度监听；重复调用会先注销上一次 */
export async function bindProgressListener(
  current: ProgressUnlisten | null,
  event: "loot-open-progress" | "loot-disenchant-progress" | "loot-upgrade-progress" | "loot-reroll-progress",
  handler: (evt: LootProgressEvent | ActionProgressEvent) => void,
): Promise<ProgressUnlisten> {
  if (current) current();
  return listen<LootProgressEvent | ActionProgressEvent>(event, (e) => {
    handler(e.payload);
  });
}
