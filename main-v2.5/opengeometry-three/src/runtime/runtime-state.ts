import type { Body } from '../bodies/body.js';
import type { Events, Listener } from '../dto/events.js';
import { OGError } from '../errors.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import type { OGMark } from '../marks/og-mark.js';
import type { LineEntry, SurfaceEntry } from '../rendering/materials/material-pool.js';
import type { TessellationProvider } from '../rendering/provider.js';
import type { RecordPool } from '../rendering/records/record-pool.js';

export type CreateOptions = { workerURL?: string | URL; tessellation?: 'worker' | 'inline' };

export type Runtime = {
  graph: OGWorldGraph;
  provider: TessellationProvider;
  records: RecordPool;
  bodies: Map<string, Body>;
  listeners: Map<Events, Set<Listener>>;
  revision: bigint;
  readyVersion: number;
  flushedReadyVersion: number;
  renderPassActive: boolean;
  displayed: Set<Body>;
  pending: Map<string, Promise<void>>;
  generations: Map<string, number>;
  buckets: Map<string, number>;
  cameraBuckets: Map<string, number>;
  failedBuckets: Set<string>;
  module: WebAssembly.Module;
  createOptions: CreateOptions;
  displayDeflection?: number;
  moving: boolean;
  poisoned: boolean;
  flushes: number;
  marks: Set<OGMark>;
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
