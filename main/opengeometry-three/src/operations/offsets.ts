import type { CurvedRegion2D, OffsetHole, OffsetPath2D, OffsetPolylineOptions } from '../dto/regions';
import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { decodeCurvedRegions, decodeOffsetPath, numberList, offsetHolesJson } from '../world-graph/codec';
import { worldGraph } from '../world-graph/world-graph-client';

const RING = 'OpenGeometry.offsetRingVariable';
const STROKE = 'OpenGeometry.offsetPolylineRegions';
const SIDE = 'OpenGeometry.offsetPolyline';
const OPTIONS = ['acuteThresholdDegrees', 'bevel', 'closed'] as const;

function checked(valid: boolean, name: string, message: string): void {
  if (!valid) throw new OGError('InvalidParameter', name, message);
}

export function offsetRingVariable(
  ring: readonly number[] | Float64Array,
  distances: readonly number[] | Float64Array,
  holes: readonly OffsetHole[] = [],
): CurvedRegion2D[] {
  const points = numberList(ring, RING, 'ring');
  const insets = numberList(distances, RING, 'distances');
  const holesJson = offsetHolesJson(holes, RING);
  return decodeCurvedRegions(call(RING, () => worldGraph().offsetRingVariable(points, insets, holesJson)), RING);
}

export function offsetPolylineRegions(
  path: readonly number[] | Float64Array,
  width: number,
  closed: boolean,
  miterLimit = 4,
): CurvedRegion2D[] {
  const points = numberList(path, STROKE, 'path');
  checked(typeof width === 'number' && typeof miterLimit === 'number', STROKE, 'width and miterLimit must be numbers');
  checked(typeof closed === 'boolean', STROKE, 'closed must be true or false');
  const text = call(STROKE, () => worldGraph().offsetPolylineRegions(points, width, closed, miterLimit));
  return decodeCurvedRegions(text, STROKE);
}

export function offsetPolyline(
  path: readonly number[] | Float64Array,
  distance: number,
  options: OffsetPolylineOptions = {},
): OffsetPath2D {
  const points = numberList(path, SIDE, 'path');
  const given: unknown = options;
  const known = (key: string): boolean => OPTIONS.some((option) => option === key);
  checked(typeof given === 'object' && given !== null && Object.keys(given).every(known), SIDE,
    'options must be { acuteThresholdDegrees?, bevel?, closed? }');
  const { acuteThresholdDegrees = 35, bevel = true, closed } = options;
  checked(typeof distance === 'number' && typeof acuteThresholdDegrees === 'number', SIDE,
    'distance and acuteThresholdDegrees must be numbers');
  checked(typeof bevel === 'boolean' && (closed === undefined || typeof closed === 'boolean'), SIDE,
    'bevel and closed must be true or false');
  const text = call(SIDE, () => worldGraph().offsetPolyline(points, distance, closed, bevel, acuteThresholdDegrees));
  return decodeOffsetPath(text, SIDE);
}
