/**
 * localStorage 延迟写入工具：避免在渲染关键路径上同步 JSON.stringify + setItem
 * 阻塞主线程。将序列化与写入推迟到浏览器空闲时段执行。
 */
export function lazySetItem(key: string, value: unknown) {
  if (typeof localStorage === "undefined") return;
  const run = () => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* ignore */
    }
  };

  // 无 window（测试/非浏览器）时同步写入，避免异步延迟导致断言读到空缓存
  if (typeof window === "undefined") {
    run();
    return;
  }

  const idle = (
    window as unknown as {
      requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => void;
    }
  ).requestIdleCallback;
  if (typeof idle === "function") {
    idle(run, { timeout: 2000 });
  } else {
    setTimeout(run, 0);
  }
}
