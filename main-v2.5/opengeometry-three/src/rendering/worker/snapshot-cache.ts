import type { OGTessellator } from '../../kernel/kernel-loader';

type Slot = { slot: number; bytes: number; lastUse: number };

export class SnapshotCache {
  private readonly entries = new Map<string, Slot>();
  private totalBytes = 0;
  private clock = 0;

  load(key: string, bytes: Uint8Array, tessellator: OGTessellator): void {
    this.drop(key, tessellator);
    const slot = tessellator.load(bytes);
    this.entries.set(key, { slot, bytes: bytes.byteLength, lastUse: ++this.clock });
    this.totalBytes += bytes.byteLength;
  }

  drop(key: string, tessellator: OGTessellator): void {
    const old = this.entries.get(key);
    if (!old) return;
    tessellator.drop(old.slot);
    this.entries.delete(key);
    this.totalBytes -= old.bytes;
  }

  touch(key: string): number | undefined {
    const entry = this.entries.get(key);
    if (!entry) return undefined;
    entry.lastUse = ++this.clock;
    return entry.slot;
  }

  evict(except: string, tessellator: OGTessellator): string[] {
    const evicted: string[] = [];
    while (this.totalBytes > 128 * 1024 * 1024) {
      const oldest = [...this.entries.entries()]
        .filter(([key]) => key !== except)
        .sort((a, b) => a[1].lastUse - b[1].lastUse)[0];
      if (!oldest) break;
      this.drop(oldest[0], tessellator);
      evicted.push(oldest[0]);
    }
    return evicted;
  }
}
