import type { Events, Listener } from '../dto/events';
import type { OGError } from '../errors';
import { compileKernel, initKernel, OGWorldGraph } from '../kernel/kernel-loader';
import { ACCURACY } from '../limits';
import { refreshDisplayed } from '../rendering/geometry/render-pass';
import { InlineBackend } from '../rendering/inline-backend';
import { createLodState } from '../rendering/lod/lod-controller';
import { RecordPool } from '../rendering/records/record-pool';
import { WorkerBackend } from '../rendering/worker-backend';
import { encode } from '../world-graph/codec';
import { emit } from './event-bus';
import {
  nextEpoch, pendingCreate, setPendingCreate, setRuntime, type CreateOptions, type Runtime,
} from './runtime-state';

export type KernelInput = { wasmURL?: string | URL; wasmModule?: WebAssembly.Module };

export function createRuntimeOnce(input: KernelInput, options: CreateOptions): Promise<Runtime> {
  const pending = pendingCreate();
  if (pending) return pending;
  const created = loadRuntime(input, options).finally(() => { setPendingCreate(undefined); });
  setPendingCreate(created);
  return created;
}

async function loadRuntime(input: KernelInput, options: CreateOptions): Promise<Runtime> {
  const wasmURL = input.wasmURL ?? new URL('./opengeometry_bg.wasm', import.meta.url);
  const module = input.wasmModule ?? await compileKernel(wasmURL);
  await initKernel(module);
  const state = createRuntime(module, options);
  setRuntime(state);
  return state;
}

export function createRuntime(module: WebAssembly.Module, options: CreateOptions): Runtime {
  const graph = new OGWorldGraph(encode(ACCURACY));
  const listeners = new Map<Events, Set<Listener>>();
  const provider = options.tessellation === 'inline' || typeof Worker === 'undefined'
    ? new InlineBackend(graph)
    : new WorkerBackend(
      graph,
      module,
      options.workerURL ?? new URL('./tessellation-worker.js', import.meta.url),
      (error) => { fallBack(state, error); },
    );
  const state: Runtime = {
    graph, provider, records: new RecordPool(), bodies: new Map(), pickTargets: new WeakMap(), byShape: new Map(),
    limbo: new Map(), listeners,
    revision: 0n, readyVersion: 0, flushedReadyVersion: -1, renderPassActive: false, pass: 0, displayed: new Map(),
    pending: new Map(), generations: new Map(), shapeBuckets: new Map(),
    lod: createLodState(() => { refreshDisplayed(state); }), module, createOptions: options, poisoned: false,
    flushes: 0, marks: new Map(), flushing: false, surfacePool: new Map(), linePool: new Map(), epoch: nextEpoch(),
  };
  return state;
}

function fallBack(state: Runtime, error: OGError): void {
  const worker = state.provider;
  state.provider = new InlineBackend(state.graph);
  worker.dispose();
  emit('error', error);
}
