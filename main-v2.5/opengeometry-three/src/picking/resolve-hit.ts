import type * as THREE from 'three';
import type { PickResult } from '../dto/pick-result.js';
import { OGError } from '../errors.js';
import { runtime } from '../runtime/runtime-state.js';

function faceRangeValue(ranges: Uint32Array, index: number): number {
  const value = ranges[index];
  if (value === undefined) throw new OGError('InvalidGeometry', 'OpenGeometry.resolveHit', 'face range out of bounds');
  return value;
}

export function resolveHit(hit: THREE.Intersection): PickResult | undefined {
  const body = [...runtime().bodies.values()]
    .find((candidate) => hit.object === candidate.surface || hit.object === candidate.outline);
  if (!body?.record) return undefined;
  const result: PickResult = { ogId: body.ogId, shapeRevision: body.record.revision };
  if (hit.object === body.surface && hit.faceIndex !== undefined) {
    for (let offset = 0; offset < body.record.faceRanges.length; offset += 3) {
      const ranges = body.record.faceRanges;
      const start = faceRangeValue(ranges, offset + 1);
      if (hit.faceIndex >= start && hit.faceIndex < start + faceRangeValue(ranges, offset + 2)) {
        result.faceId = faceRangeValue(ranges, offset);
        break;
      }
    }
  } else if (hit.index !== undefined) Object.assign(result, { edgeId: body.record.edgeIds[Math.floor(hit.index / 2)] });
  return result;
}
