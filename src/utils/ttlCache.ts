/** 带 TTL 与容量上限的进程内缓存（Map 插入序 + get 提升 = 近似 LRU） */
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
    // 提升到末尾，避免热 key 因插入序靠前被误淘汰
    this.map.delete(key);
    this.map.set(key, cached);
    return cached.data;
  }

  set(key: string, data: T): void {
    if (this.map.has(key)) {
      this.map.delete(key);
    } else if (this.map.size >= this.maxSize) {
      const firstKey = this.map.keys().next().value;
      if (firstKey) this.map.delete(firstKey);
    }
    this.map.set(key, { data, timestamp: Date.now() });
  }

  clear(): void {
    this.map.clear();
  }
}
