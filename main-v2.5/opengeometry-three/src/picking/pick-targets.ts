import type * as THREE from 'three';
import type { Body } from '../bodies/body';
import { OGError } from '../errors';
import type { GeometryRecord } from '../rendering/records/geometry-record';
import { runtime } from '../runtime/runtime-state';

function faceRangeValue(ranges: Uint32Array, index: number): number {
  const value = ranges[index];
  if (value === undefined) throw new OGError('InvalidGeometry', 'OpenGeometry.resolveHit', 'face range out of bounds');
  return value;
}

export function pickTarget(object: THREE.Object3D): Body | undefined {
  return runtime().pickTargets.get(object);
}

export function faceIdAt(ranges: Uint32Array, faceIndex: number): number | undefined {
  const length = ranges.length;
  if (length % 3 !== 0) throw new OGError('InvalidGeometry', 'OpenGeometry.resolveHit', 'face ranges are not triples');
  let low = 0;
  let high = length / 3;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (faceRangeValue(ranges, 3 * middle + 1) <= faceIndex) low = middle + 1;
    else high = middle;
  }
  if (low === 0) return undefined;
  const offset = 3 * (low - 1);
  const start = faceRangeValue(ranges, offset + 1);
  return faceIndex < start + faceRangeValue(ranges, offset + 2) ? faceRangeValue(ranges, offset) : undefined;
}

export function edgeIdAt(record: GeometryRecord, index: number): number | undefined {
  return record.edgeIds[Math.floor(index / 2)];
}
