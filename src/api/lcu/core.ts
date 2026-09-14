import { invoke } from "@tauri-apps/api/core";

/** lol_path 列表中的特殊标记：表示该条目为「启动 WeGame」而非真实客户端路径（与 Rust 侧 config::WEGAME_MARKER 保持一致） */
export const WEGAME_MARKER = "WeGame";

export interface LcuApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

/**
 * 净化错误消息，若包含 JSON 则尝试解析提取 message，若有“LCU 返回错误”前缀则过滤
 */
export function cleanError(error: unknown): string {
  let msg = error instanceof Error ? error.message : String(error);

  // 1. 优先使用正则表达式从可能含有 JSON (甚至是残破 JSON) 的文本中，安全且精准地提取核心 message 字段
  const jsonMessageMatch = msg.match(/"(?:error)?Message"\s*:\s*"([^"]+)"/i);
  if (jsonMessageMatch && jsonMessageMatch[1]) {
    let cleanMsg = jsonMessageMatch[1];
    if (cleanMsg.includes("Error response for ") && cleanMsg.includes(":")) {
      const parts = cleanMsg.split(":");
      cleanMsg = parts[parts.length - 1].trim();
    }
    msg = cleanMsg;
  } else if (msg.includes("{") && msg.includes("}")) {
    // 降级：如果正则未匹配上但含有完整大括号，依然尝试标准 JSON 序列化解析
    try {
      const start = msg.indexOf("{");
      const end = msg.lastIndexOf("}");
      const jsonStr = msg.slice(start, end + 1);
      const obj = JSON.parse(jsonStr);
      let cleanMsg = obj.message || obj.errorMessage || obj.description;
      if (cleanMsg) {
        if (
          cleanMsg.includes("Error response for ") &&
          cleanMsg.includes(":")
        ) {
          const parts = cleanMsg.split(":");
          cleanMsg = parts[parts.length - 1].trim();
        }
        msg = cleanMsg;
      }
    } catch {
      /* ignore */
    }
  }

  // 2. 如果包含“LCU 返回错误”前缀，提取后面的纯消息
  if (msg.startsWith("LCU 返回错误")) {
    const parts = msg.split("]:");
    if (parts.length > 1) {
      msg = parts[1].trim();
    }
  }

  // 3. 仅安全剥离最外层可能残留的包围单双引号，不损伤任何合法的 {} 符号
  msg = msg.trim().replace(/^["']+|["']+$/g, "");

  // 4. 友好汉化经典的观战业务报错
  if (msg.includes("Already in gameflow")) {
    return "你当前已处于对局中，无法重复观战";
  }
  if (msg.includes("Cannot spectate game because spectator key is missing")) {
    return "该召唤师当前不在游戏中（观战密钥缺失）";
  }

  return msg;
}

/**
 * 统一的 LCU API 调用封装。
 * 所有请求通过 Tauri IPC 转发到 Rust 侧的 call_lcu_api 命令。
 */
export async function lcuRequest<T>(
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE",
  path: string,
  body?: unknown,
): Promise<LcuApiResponse<T>> {
  try {
    const data = await invoke<T>("call_lcu_api", { method, path, body });
    return { success: true, data };
  } catch (error: unknown) {
    return { success: false, error: cleanError(error) };
  }
}
