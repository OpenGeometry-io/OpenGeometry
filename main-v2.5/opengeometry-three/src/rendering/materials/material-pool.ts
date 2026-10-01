import * as THREE from 'three';
import { runtime } from '../../runtime/runtime-state.js';
import { acquire, release, type PoolEntry, type PooledMaterial } from './ref-counted-pool.js';

export type SurfaceAppearance = { color: number; opacity: number };
export type SurfaceEntry = PoolEntry<THREE.MeshBasicMaterial>;
export type LineEntry = PoolEntry<THREE.LineBasicMaterial>;

export function surfaceMaterial(value: SurfaceAppearance): PooledMaterial<THREE.MeshBasicMaterial> {
  const key = `${String(value.color)}:${String(value.opacity)}`;
  return acquire(runtime().surfacePool, key, () => new THREE.MeshBasicMaterial({
    color: value.color, opacity: value.opacity, transparent: value.opacity < 1, side: THREE.FrontSide,
    polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1,
  }));
}

export function lineMaterial(color: number): PooledMaterial<THREE.LineBasicMaterial> {
  const key = String(color);
  return acquire(runtime().linePool, key, () => new THREE.LineBasicMaterial({ color }));
}

export function releaseSurface(key: string): void {
  release(runtime().surfacePool, key);
}

export function releaseLine(key: string): void {
  release(runtime().linePool, key);
}
