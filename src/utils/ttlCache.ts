/** 带 TTL 与容量上限的进程内缓存（插入序近似 LRU） */
export class TtlCache<T> {
  private map = new Map<string, { data: T; timestamp: number }>();

  constructor(
    private ttlMs: number,
    private maxSize: number,
  ) {}

  get(key: string): T | null {
    const cached = this.map.get(key);
    if (!cached) return null;
    if (Date.now() - cached.timestamp >= this.ttlMs) {
      this.map.delete(key);
      return null;
    }
    return cached.data;
  }

  set(key: string, data: T): void {
    if (this.map.size >= this.maxSize) {
      const firstKey = this.map.keys().next().value;
      if (firstKey) this.map.delete(firstKey);
    }
    this.map.set(key, { data, timestamp: Date.now() });
  }

  clear(): void {
    this.map.clear();
  }
}
