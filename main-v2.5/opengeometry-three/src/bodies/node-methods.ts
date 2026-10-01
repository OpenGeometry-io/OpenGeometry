import type { Bounds } from '../dto/bounds.js';
import type { Brep } from '../dto/brep.js';
import type { Placement } from '../dto/placement.js';
import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { runtime } from '../runtime/runtime-state.js';
import {
  decodeBounds, decodeBrep, decodeChildren, decodeParent, decodePlacement, encode,
} from '../world-graph/codec.js';
import { worldGraph } from '../world-graph/world-graph-client.js';

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

export function getBrep(owner: string, ogId: string): Brep {
  const label = `${owner}.getBrep`;
  return decodeBrep(call(label, () => worldGraph().brep(ogId)), label);
}

export function getInstanceCount(owner: string, ogId: string): number {
  return call(`${owner}.getInstanceCount`, () => worldGraph().instanceCount(ogId));
}
