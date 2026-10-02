import * as THREE from 'three';
import { runtime } from '../../runtime/runtime-state';
import { acquire, release, type PoolEntry, type PooledMaterial } from './ref-counted-pool';

export type MaterialAppearance = { color: number; opacity: number };
export type SurfaceEntry = PoolEntry<THREE.MeshStandardMaterial>;
export type LineEntry = PoolEntry<THREE.LineBasicMaterial>;

function styleKey(value: MaterialAppearance): string {
  return `${String(value.color)}:${String(value.opacity)}`;
}

export function surfaceMaterial(value: MaterialAppearance): PooledMaterial<THREE.MeshStandardMaterial> {
  return acquire(runtime().surfacePool, styleKey(value), () => new THREE.MeshStandardMaterial({
    color: value.color, opacity: value.opacity, transparent: value.opacity < 1, side: THREE.FrontSide,
    polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1,
  }));
}

export function lineMaterial(value: MaterialAppearance): PooledMaterial<THREE.LineBasicMaterial> {
  return acquire(runtime().linePool, styleKey(value), () => new THREE.LineBasicMaterial({
    color: value.color, opacity: value.opacity, transparent: value.opacity < 1,
  }));
}

export function releaseSurface(key: string): void {
  release(runtime().surfacePool, key);
}

export function releaseLine(key: string): void {
  release(runtime().linePool, key);
}
