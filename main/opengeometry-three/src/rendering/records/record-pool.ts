import * as THREE from 'three';
import type { Bounds } from '../../dto/bounds';
import type { GeometryRecord } from './geometry-record';

const IDLE_BYTES = 256 * 1024 * 1024;
const IDLE_PER_SHAPE = 2;

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
  private byShape = new Map<string, Set<GeometryRecord>>();
  private idle = new Set<GeometryRecord>();
  private idleBytes = 0;
  private placeholders = new Map<string, Placeholder>();
  private placeholderOf = new Map<THREE.BufferGeometry, Placeholder>();
  private clock = 0;

  get(shapeId: string, revision: number, bucket: number): GeometryRecord | undefined {
    const record = this.records.get(`${shapeId}@${String(revision)}#${String(Math.log2(bucket))}`);
    if (record) this.touch(record);
    return record;
  }

  put(record: GeometryRecord): GeometryRecord {
    const previous = this.records.get(record.key);
    if (previous) {
      record.dispose();
      return previous;
    }
    this.records.set(record.key, record);
    this.byShape.set(record.shapeId, (this.byShape.get(record.shapeId) ?? new Set()).add(record));
    this.setIdle(record, true);
    this.touch(record);
    this.evict(record.shapeId);
    return record;
  }

  retain(record: GeometryRecord): void {
    record.refs++;
    this.setIdle(record, false);
    this.touch(record);
  }

  release(record: GeometryRecord): void {
    record.refs = Math.max(0, record.refs - 1);
    if (!this.owns(record)) return;
    if (record.refs === 0) this.setIdle(record, true);
    this.touch(record);
    this.evict(record.shapeId);
  }

  purge(shapeId: string, revision?: number): void {
    for (const record of [...this.byShape.get(shapeId) ?? []]) {
      if ((revision === undefined || record.revision !== revision) && record.refs === 0) this.drop(record);
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
    this.byShape.clear();
    this.idle.clear();
    this.idleBytes = 0;
    for (const entry of this.placeholders.values()) entry.geometry.dispose();
    this.placeholders.clear();
    this.placeholderOf.clear();
  }

  private owns(record: GeometryRecord): boolean {
    return this.records.get(record.key) === record;
  }

  private touch(record: GeometryRecord): void {
    record.lastUse = ++this.clock;
    if (this.idle.delete(record)) this.idle.add(record);
  }

  private setIdle(record: GeometryRecord, idle: boolean): void {
    if (idle === this.idle.has(record) || (idle && !this.owns(record))) return;
    if (idle) this.idle.add(record);
    else this.idle.delete(record);
    this.idleBytes += idle ? record.bytes : -record.bytes;
  }

  private drop(record: GeometryRecord): void {
    record.dispose();
    if (this.owns(record)) this.records.delete(record.key);
    const shape = this.byShape.get(record.shapeId);
    shape?.delete(record);
    if (shape?.size === 0) this.byShape.delete(record.shapeId);
    this.setIdle(record, false);
  }

  private evict(shapeId: string): void {
    const idle = [...this.byShape.get(shapeId) ?? []].filter((record) => record.refs === 0);
    idle.sort((a, b) => a.lastUse - b.lastUse);
    for (const record of idle.slice(0, Math.max(0, idle.length - IDLE_PER_SHAPE))) this.drop(record);
    for (const record of this.idle) {
      if (this.idleBytes <= IDLE_BYTES) break;
      this.drop(record);
    }
  }
}
