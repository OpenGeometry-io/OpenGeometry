import type { NodeInfo } from '../dto/node-info';
import type { OGWorldGraph } from '../kernel/kernel-loader';
import { call } from '../kernel/kernel-session';
import { runtime } from '../runtime/runtime-state';
import { decodeCreation, decodeDisplayBuckets, decodeNodeInfo } from './codec';

export function worldGraph(): OGWorldGraph {
  return runtime().graph;
}

export function node(ogId: string, label: string): NodeInfo {
  return decodeNodeInfo(call(label, () => worldGraph().node(ogId)), label);
}

export function checkHandle(label: string, target: { handle: number; generation: number }): void {
  call(label, () => worldGraph().nodeByHandle(target.handle, target.generation));
}

export function creation(value: string, label: string): string {
  return decodeCreation(value, label).ogId;
}

export function displayBuckets(ogId: string, label: string): { floor: number; static: number } {
  return decodeDisplayBuckets(call(label, () => worldGraph().displayBuckets(ogId)), label);
}
