import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { cleanError } from "../../api/lcu";

describe("cleanError", () => {
  it("returns plain string errors as-is", () => {
    expect(cleanError("LCU 未连接")).toBe("LCU 未连接");
  });

  it("extracts message from Error", () => {
    expect(cleanError(new Error("something failed"))).toBe("something failed");
  });

  it("extracts message field from embedded JSON", () => {
    const raw = 'prefix {"message":"召唤师不存在"} suffix';
    expect(cleanError(raw)).toBe("召唤师不存在");
  });

  it("extracts errorMessage field", () => {
    const raw = '{"errorMessage":"queue disabled"}';
    expect(cleanError(raw)).toBe("queue disabled");
  });

  it("strips LCU 返回错误 prefix", () => {
    expect(cleanError("LCU 返回错误 [404]: not found")).toBe("not found");
  });

  it("maps spectator already in gameflow", () => {
    expect(cleanError("Already in gameflow")).toBe("你当前已处于对局中，无法重复观战");
  });

  it("maps missing spectator key", () => {
    expect(
      cleanError("Cannot spectate game because spectator key is missing"),
    ).toBe("该召唤师当前不在游戏中（观战密钥缺失）");
  });

  it("strips surrounding quotes", () => {
    expect(cleanError('"boom"')).toBe("boom");
  });
});
