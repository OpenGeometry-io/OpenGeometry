import type { Bounds } from '../dto/bounds';
import type { Brep } from '../dto/brep';
import type { Measure, Validity } from '../dto/measure';
import type { MirrorPlane } from '../dto/mirror';
import type { Placement } from '../dto/placement';
import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { flush } from '../rendering/geometry/geometry-scheduler';
import { runtime } from '../runtime/runtime-state';
import {
  decodeBounds, decodeBrep, decodeChildren, decodeParent, decodePlacement, encode, scope,
} from '../world-graph/codec';
import { worldGraph } from '../world-graph/world-graph-client';

export function checkNode(label: string, noun: string, epoch: number, handle: number, generation: number): void {
  if (runtime().epoch !== epoch) throw new OGError('Disposed', label, `${noun} belongs to a reset runtime`);
  call(label, () => worldGraph().nodeByHandle(handle, generation));
}

export function getPlacement(owner: string, ogId: string): Placement {
  const label = `${owner}.getPlacement`;
  return decodePlacement(call(label, () => worldGraph().placement(ogId)), label);
}

export function getWorldPlacement(owner: string, ogId: string): Placement {
  const label = `${owner}.getWorldPlacement`;
  return decodePlacement(call(label, () => worldGraph().worldPlacement(ogId)), label);
}

export function addChild(owner: string, ogId: string, children: { ogId: string }[], keepWorld: boolean): void {
  call(`${owner}.addChild`, () => worldGraph().addChild(ogId, encode(children.map((child) => child.ogId)), keepWorld));
}

export function removeChild(owner: string, ogId: string, childId: string, keepWorld: boolean): void {
  call(`${owner}.removeChild`, () => worldGraph().removeChild(ogId, childId, keepWorld));
}

export function getChildren(owner: string, ogId: string): string[] {
  const label = `${owner}.getChildren`;
  return decodeChildren(call(label, () => worldGraph().children(ogId)), label);
}

export function getParent(owner: string, ogId: string): string | null {
  const label = `${owner}.getParent`;
  return decodeParent(call(label, () => worldGraph().parent(ogId)), label);
}

export function getBounds(owner: string, ogId: string): Bounds | null {
  const label = `${owner}.getBounds`;
  return decodeBounds(call(label, () => worldGraph().bounds(ogId)), label);
}

export function localBounds(label: string, ogId: string): Bounds | null {
  return decodeBounds(call(label, () => worldGraph().localBounds(ogId)), label);
}

export function getBrep(owner: string, ogId: string): Brep {
  const label = `${owner}.getBrep`;
  return decodeBrep(call(label, () => worldGraph().brep(ogId)), label);
}

export function getWorldBrep(owner: string, ogId: string): Brep {
  const label = `${owner}.getWorldBrep`;
  return decodeBrep(call(label, () => worldGraph().worldBrep(ogId)), label);
}

export function faceNormalAt(
  owner: string, ogId: string, faceId: number, point: [number, number, number],
): [number, number, number] {
  const label = `${owner}.faceNormalAt`;
  if (!Number.isInteger(faceId) || faceId < 0 || faceId > 0xffffffff) {
    throw new OGError('InvalidParameter', label, 'faceId must be an unsigned integer');
  }
  const [x = Number.NaN, y = Number.NaN, z = Number.NaN] = call(
    label, () => worldGraph().faceNormalAt(ogId, faceId, Float64Array.from(point)),
  );
  return [x, y, z];
}

export type PointClassification = 'Inside' | 'Outside' | 'Boundary' | 'Unknown';

export function classifyPoint(owner: string, ogId: string, point: [number, number, number]): PointClassification {
  const label = `${owner}.classifyPoint`;
  const answer = call(label, () => worldGraph().classifyPoint(ogId, Float64Array.from(point)));
  if (answer === 'Inside' || answer === 'Outside' || answer === 'Boundary' || answer === 'Unknown') return answer;
  throw new OGError('InvalidGeometry', label, `malformed classification ${answer}`);
}

export function getInstanceCount(owner: string, ogId: string): number {
  return call(`${owner}.getInstanceCount`, () => worldGraph().instanceCount(ogId));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isValidity(value: unknown): value is Validity {
  if (!isRecord(value) || typeof value['valid'] !== 'boolean' || typeof value['closed'] !== 'boolean') return false;
  const { bodyType, error } = value;
  const typed = bodyType === null || bodyType === 'Wire' || bodyType === 'Sheet' || bodyType === 'Solid';
  const reported = isRecord(error) && typeof error['code'] === 'string' && typeof error['message'] === 'string';
  return typed && (error === undefined || reported);
}

export function measure(owner: string, ogId: string, options: { deflection?: number }): Measure {
  const label = `${owner}.measure`;
  const { deflection } = options;
  if (deflection !== undefined && typeof deflection !== 'number') {
    throw new OGError('InvalidParameter', label, 'deflection must be a number');
  }
  const [
    volume = Number.NaN, area = Number.NaN, errorBound = Number.NaN, achieved = Number.NaN, areaErrorBound = Number.NaN,
  ] = call(label, () => worldGraph().measure(ogId, deflection));
  return { volume, area, errorBound, areaErrorBound, deflection: achieved };
}

export function validity(owner: string, ogId: string): Validity {
  const label = `${owner}.validity`;
  const value = call(label, (): unknown => worldGraph().validity(ogId));
  if (isValidity(value)) return value;
  throw new OGError('InvalidGeometry', label, 'malformed validity');
}

export function mirror(owner: string, ogId: string, plane: MirrorPlane, options: { instances?: 'all' }): void {
  call(`${owner}.mirror`, () => worldGraph().mirror(ogId, encode(plane), scope(options)));
  flush();
}
