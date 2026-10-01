import type { Events, Listener } from '../dto/events.js';
import { compileKernel, initKernel, OGWorldGraph } from '../kernel/kernel-loader.js';
import { ACCURACY } from '../limits.js';
import { InlineBackend } from '../rendering/inline-backend.js';
import { RecordPool } from '../rendering/records/record-pool.js';
import { WorkerBackend } from '../rendering/worker-backend.js';
import { encode } from '../world-graph/codec.js';
import { emit } from './event-bus.js';
import {
  nextEpoch, pendingCreate, setPendingCreate, setRuntime, type CreateOptions, type Runtime,
} from './runtime-state.js';

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
      (error) => { emit('error', error); },
    );
  const state: Runtime = {
    graph, provider, records: new RecordPool(), bodies: new Map(), listeners,
    revision: 0n, readyVersion: 0, flushedReadyVersion: -1, renderPassActive: false, displayed: new Set(),
    pending: new Map(), generations: new Map(), buckets: new Map(), cameraBuckets: new Map(), failedBuckets: new Set(),
    module, createOptions: options, moving: false, poisoned: false, flushes: 0, marks: new Set(),
    flushing: false, surfacePool: new Map(), linePool: new Map(), epoch: nextEpoch(),
  };
  return state;
}
