import type { CurvedRegion2D, OffsetRegion, RegionOperation } from '../dto/regions';
import { call } from '../kernel/kernel-session';
import {
  curvedRegionsJson, decodeCurvedRegions, decodeRegions, regionOperation, regionsJson,
} from '../world-graph/codec';
import { worldGraph } from '../world-graph/world-graph-client';

const PLANAR = 'OpenGeometry.booleanRegions2D';
const CURVED = 'OpenGeometry.booleanCurvedRegions2D';

export function booleanRegions2D(
  a: readonly OffsetRegion[],
  b: readonly OffsetRegion[],
  operation: RegionOperation,
): OffsetRegion[] {
  const kind = regionOperation(operation, PLANAR);
  const aJson = regionsJson(a, PLANAR);
  const bJson = regionsJson(b, PLANAR);
  return decodeRegions(call(PLANAR, () => worldGraph().booleanRegions2D(aJson, bJson, kind)), PLANAR);
}

export function booleanCurvedRegions2D(
  a: readonly CurvedRegion2D[],
  b: readonly CurvedRegion2D[],
  operation: RegionOperation,
): CurvedRegion2D[] {
  const kind = regionOperation(operation, CURVED);
  const aJson = curvedRegionsJson(a, CURVED);
  const bJson = curvedRegionsJson(b, CURVED);
  const text = call(CURVED, () => worldGraph().booleanCurvedRegions2D(aJson, bJson, kind));
  return decodeCurvedRegions(text, CURVED);
}
