import type { Creation } from '../dto/creation.js';
import type { NodeInfo } from '../dto/node-info.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import { call } from '../kernel/kernel-session.js';
import { runtime } from '../runtime/runtime-state.js';

export function worldGraph(): OGWorldGraph {
  return runtime().graph;
}

export function node(ogId: string): NodeInfo {
  return call('node', () => JSON.parse(worldGraph().node(ogId)) as NodeInfo);
}

export function creation(value: string): string {
  return (JSON.parse(value) as Creation).ogId;
}
