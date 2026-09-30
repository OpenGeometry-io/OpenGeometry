import type { Body } from '../bodies/body.js';
import type { Events, Listener } from '../dto/events.js';
import { OGError } from '../errors.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import type { OGMark } from '../marks/og-mark.js';
import type { LineEntry, SurfaceEntry } from '../rendering/materials/material-pool.js';
import type { TessellationProvider } from '../rendering/provider.js';
import type { RecordPool } from '../rendering/records/record-pool.js';

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
  displayDeflection?: number;
  moving: boolean;
  poisoned: boolean;
  flushes: number;
  marks: Set<OGMark>;
};

let state: Runtime | undefined;

export const SURFACE_POOL = new Map<string, SurfaceEntry>();
export const LINE_POOL = new Map<string, LineEntry>();

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
