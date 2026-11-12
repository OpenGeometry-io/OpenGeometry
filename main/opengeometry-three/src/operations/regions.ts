import type { CurvedRegion2D, OffsetRegion, RegionOperation } from '../dto/regions';
import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import {
  curvedRegionsJson, decodeCurvedRegions, decodeRegions, encode, regionOperation, regionsJson,
} from '../world-graph/codec';
import { worldGraph } from '../world-graph/world-graph-client';

const PLANAR = 'OpenGeometry.booleanRegions2D';
const CURVED = 'OpenGeometry.booleanCurvedRegions2D';
const INSET = 'OpenGeometry.insetCurvedRegion2D';

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

export function insetCurvedRegion2D(region: CurvedRegion2D, distance: number): CurvedRegion2D[] {
  const given: unknown = region;
  if (typeof given !== 'object' || given === null || Array.isArray(given)) {
    throw new OGError('InvalidParameter', INSET, 'region must be one { outer, holes } of line and arc edges');
  }
  if (typeof distance !== 'number') throw new OGError('InvalidParameter', INSET, 'distance must be a number');
  const regionJson = encode(region);
  return decodeCurvedRegions(call(INSET, () => worldGraph().insetCurvedRegion2D(regionJson, distance)), INSET);
}
