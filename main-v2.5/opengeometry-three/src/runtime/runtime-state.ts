import type { Body } from '../bodies/body.js';
import type { Events, Listener } from '../dto/events.js';
import { OGError } from '../errors.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import type { OGMark } from '../marks/og-mark.js';
import type { LodState } from '../rendering/lod/lod-controller.js';
import type { LineEntry, SurfaceEntry } from '../rendering/materials/material-pool.js';
import type { TessellationProvider } from '../rendering/provider.js';
import type { RecordPool } from '../rendering/records/record-pool.js';

export type CreateOptions = { workerURL?: string | URL; tessellation?: 'worker' | 'inline' };

export type LimboEntry = { watermark: number; parent: Body['parent'] };

export type Runtime = {
  graph: OGWorldGraph;
  provider: TessellationProvider;
  records: RecordPool;
  bodies: Map<string, Body>;
  byShape: Map<string, Set<Body>>;
  limbo: Map<Body, LimboEntry>;
  listeners: Map<Events, Set<Listener>>;
  revision: bigint;
  readyVersion: number;
  flushedReadyVersion: number;
  renderPassActive: boolean;
  pass: number;
  displayed: Map<Body, number>;
  pending: Map<string, Promise<void>>;
  generations: Map<string, number>;
  shapeBuckets: Map<string, { revision: number; floor: number; static: number; error?: unknown }>;
  lod: LodState;
  module: WebAssembly.Module;
  createOptions: CreateOptions;
  displayDeflection?: number;
  poisoned: boolean;
  flushes: number;
  marks: Map<number, OGMark>;
  flushing: boolean;
  surfacePool: Map<string, SurfaceEntry>;
  linePool: Map<string, LineEntry>;
  epoch: number;
};

let state: Runtime | undefined;
let creating: Promise<Runtime> | undefined;
let epochs = 0;

export function runtime(): Runtime {
  if (!state) throw new OGError('NotInitialised', 'OpenGeometry', 'call OpenGeometry.create() first');
  return state;
}

export function currentRuntime(): Runtime | undefined {
  return state;
}

export function setRuntime(value: Runtime | undefined): void {
  state = value;
}

export function pendingCreate(): Promise<Runtime> | undefined {
  return creating;
}

export function setPendingCreate(value: Promise<Runtime> | undefined): void {
  creating = value;
}

export function nextEpoch(): number {
  return ++epochs;
}

export function bodyKey(handle: number, generation: number): string {
  return `${String(handle)}:${String(generation)}`;
}

export function reindexShape(state: Runtime, body: Body, previousShapeId: string | null): void {
  const shapeId = body.lastInfo.shapeId;
  if (shapeId === previousShapeId) return;
  if (previousShapeId) {
    const previous = state.byShape.get(previousShapeId);
    previous?.delete(body);
    if (previous?.size === 0) {
      state.byShape.delete(previousShapeId);
      forgetShape(state, previousShapeId);
    }
  }
  if (shapeId) state.byShape.set(shapeId, (state.byShape.get(shapeId) ?? new Set<Body>()).add(body));
}

export function releaseShapeIfEmpty(state: Runtime, shapeId: string, revision: number): void {
  if (state.byShape.get(shapeId)?.size) return;
  state.byShape.delete(shapeId);
  forgetShape(state, shapeId);
  state.records.purge(shapeId);
  state.provider.drop(shapeId, revision);
}

function forgetShape(state: Runtime, shapeId: string): void {
  state.lod.shapes.delete(shapeId);
  state.shapeBuckets.delete(shapeId);
}
