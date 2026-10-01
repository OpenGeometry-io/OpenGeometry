import type { Placement } from '../dto/placement.js';
import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { runtime } from '../runtime/runtime-state.js';
import { encode } from '../world-graph/codec.js';
import { worldGraph } from '../world-graph/world-graph-client.js';

export function checkNode(owner: string, noun: string, epoch: number, handle: number, generation: number): void {
  if (runtime().epoch !== epoch) throw new OGError('Disposed', owner, `${noun} belongs to a reset runtime`);
  call(owner, () => worldGraph().nodeByHandle(handle, generation));
}

export function getPlacement(owner: string, ogId: string): Placement {
  return JSON.parse(call(`${owner}.getPlacement`, () => worldGraph().placement(ogId)));
}

export function getWorldPlacement(owner: string, ogId: string): Placement {
  return JSON.parse(call(`${owner}.getWorldPlacement`, () => worldGraph().worldPlacement(ogId)));
}

export function addChild(owner: string, ogId: string, children: { ogId: string }[], keepWorld: boolean): void {
  call(`${owner}.addChild`, () => worldGraph().addChild(ogId, encode(children.map((child) => child.ogId)), keepWorld));
}

export function removeChild(owner: string, ogId: string, childId: string, keepWorld: boolean): void {
  call(`${owner}.removeChild`, () => worldGraph().removeChild(ogId, childId, keepWorld));
}

export function getChildren(owner: string, ogId: string): string[] {
  return JSON.parse(call(`${owner}.getChildren`, () => worldGraph().children(ogId)));
}

export function getParent(owner: string, ogId: string): string | null {
  return JSON.parse(call(`${owner}.getParent`, () => worldGraph().parent(ogId)));
}

export function getBounds(owner: string, ogId: string): [number, number, number, number, number, number] | null {
  return JSON.parse(call(`${owner}.getBounds`, () => worldGraph().bounds(ogId)));
}

export function getBrep(owner: string, ogId: string) {
  return JSON.parse(call(`${owner}.getBrep`, () => worldGraph().brep(ogId)));
}

export function getInstanceCount(owner: string, ogId: string): number {
  return call(`${owner}.getInstanceCount`, () => worldGraph().instanceCount(ogId));
}
