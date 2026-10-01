import * as THREE from 'three';
import type { Bounds } from '../../dto/bounds.js';
import type { GeometryRecord } from './geometry-record.js';

type Placeholder = { key: string; geometry: THREE.BufferGeometry; holders: number };

function placeholderGeometry(bounds: Bounds | null): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(0), 3));
  geometry.boundingBox = bounds ? new THREE.Box3().setFromArray(bounds) : new THREE.Box3();
  geometry.boundingSphere = geometry.boundingBox.getBoundingSphere(new THREE.Sphere());
  return geometry;
}

export class RecordPool {
  private records = new Map<string, GeometryRecord>();
  private placeholders = new Map<string, Placeholder>();
  private placeholderOf = new Map<THREE.BufferGeometry, Placeholder>();
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

  placeholder(shapeId: string, revision: number, bounds: () => Bounds | null): THREE.BufferGeometry {
    const key = `${shapeId}@${String(revision)}`;
    let entry = this.placeholders.get(key);
    if (!entry) {
      entry = { key, geometry: placeholderGeometry(bounds()), holders: 0 };
      this.placeholders.set(key, entry);
      this.placeholderOf.set(entry.geometry, entry);
    }
    entry.holders++;
    return entry.geometry;
  }

  releasePlaceholder(geometry: THREE.BufferGeometry): boolean {
    const entry = this.placeholderOf.get(geometry);
    if (!entry) return false;
    if (--entry.holders <= 0) {
      geometry.dispose();
      this.placeholders.delete(entry.key);
      this.placeholderOf.delete(geometry);
    }
    return true;
  }

  clear(): void {
    for (const record of this.records.values()) record.dispose();
    this.records.clear();
    for (const entry of this.placeholders.values()) entry.geometry.dispose();
    this.placeholders.clear();
    this.placeholderOf.clear();
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
