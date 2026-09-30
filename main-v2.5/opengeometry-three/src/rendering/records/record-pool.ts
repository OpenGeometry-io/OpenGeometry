import type { GeometryRecord } from './geometry-record.js';

export class RecordPool {
  private records = new Map<string, GeometryRecord>();
  private clock = 0;

  get(shapeId: string, revision: number, bucket: number): GeometryRecord | undefined {
    const record = this.records.get(`${shapeId}@${String(revision)}#${String(Math.log2(bucket))}`);
    if (record) record.lastUse = ++this.clock;
    return record;
  }

  put(record: GeometryRecord): GeometryRecord {
    const previous = this.records.get(record.key);
    if (previous) {
      record.dispose();
      return previous;
    }
    record.lastUse = ++this.clock;
    this.records.set(record.key, record);
    this.evict();
    return record;
  }

  retain(record: GeometryRecord): void {
    record.refs++;
    record.lastUse = ++this.clock;
  }

  release(record: GeometryRecord): void {
    record.refs = Math.max(0, record.refs - 1);
    record.lastUse = ++this.clock;
    this.evict();
  }

  purge(shapeId: string, revision?: number): void {
    for (const [key, record] of this.records) {
      if (record.shapeId === shapeId && (revision === undefined || record.revision !== revision) && record.refs === 0) {
        record.dispose();
        this.records.delete(key);
      }
    }
  }

  clear(): void {
    for (const record of this.records.values()) record.dispose();
    this.records.clear();
  }

  private evict(): void {
    const idle = [...this.records.values()].filter((record) => record.refs === 0).sort((a, b) => a.lastUse - b.lastUse);
    const perShape = new Map<string, number>();
    let total = idle.reduce((sum, record) => sum + record.bytes, 0);
    for (const record of idle.slice().reverse()) perShape.set(record.shapeId, (perShape.get(record.shapeId) ?? 0) + 1);
    for (const record of idle) {
      const count = perShape.get(record.shapeId) ?? 0;
      if (count <= 2 && total <= 256 * 1024 * 1024) continue;
      record.dispose();
      this.records.delete(record.key);
      total -= record.bytes;
      perShape.set(record.shapeId, count - 1);
    }
  }
}
