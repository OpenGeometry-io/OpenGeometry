import type { NodeInfo } from '../dto/node-info.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import { call } from '../kernel/kernel-session.js';
import { runtime } from '../runtime/runtime-state.js';
import { decodeCreation, decodeDisplayBuckets, decodeNodeInfo } from './codec.js';

export function worldGraph(): OGWorldGraph {
  return runtime().graph;
}

export function node(ogId: string, label: string): NodeInfo {
  return decodeNodeInfo(call(label, () => worldGraph().node(ogId)), label);
}

export function creation(value: string, label: string): string {
  return decodeCreation(value, label).ogId;
}

export function displayBuckets(ogId: string, label: string): { floor: number; static: number } {
  return decodeDisplayBuckets(call(label, () => worldGraph().displayBuckets(ogId)), label);
}
