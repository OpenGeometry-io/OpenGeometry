import type * as THREE from 'three';
import type { PickResult } from '../dto/pick-result.js';
import { edgeIdAt, faceIdAt, pickTarget } from './pick-targets.js';

export function resolveHit(hit: THREE.Intersection): PickResult | undefined {
  const body = pickTarget(hit.object);
  if (!body?.record) return undefined;
  const result: PickResult = { ogId: body.ogId, shapeRevision: body.record.revision };
  if (hit.object === body.surface && hit.faceIndex !== undefined) {
    const faceId = faceIdAt(body.record.faceRanges, hit.faceIndex);
    if (faceId !== undefined) result.faceId = faceId;
  } else if (hit.index !== undefined) Object.assign(result, { edgeId: edgeIdAt(body.record, hit.index) });
  return result;
}
