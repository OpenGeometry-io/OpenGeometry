import type { DisplayBuffers } from '../dto/display-buffers.js';
import { OGError } from '../errors.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import { call } from '../kernel/kernel-session.js';
import { Deferred } from './deferred.js';
import { InlineBackend } from './inline-backend.js';
import type { TessellationProvider, TessellationRequest } from './provider.js';
import type { SnapshotMessage, TessellateMessage, WorkerMessage, WorkerReply } from './worker-protocol.js';

export type WorkerSend = { kind: 'tessellate'; shapeId: string; bucket: number; priority: number; generation: number };

type SendBody = Omit<SnapshotMessage, 'request'> | Omit<TessellateMessage, 'request'>;
type PendingSend = {
  value: TessellationRequest | undefined;
  resolve: (reply: WorkerReply | undefined) => void;
  reject: (error: unknown) => void;
};

const SEND_LOG_LIMIT = 1_000;

export class WorkerBackend implements TessellationProvider {
  readonly activeBackend = 'worker' as const;
  readonly sendLog: WorkerSend[] = [];
  private live: Worker | undefined;
  private nextRequest = 1;
  private pending = new Map<number, PendingSend>();
  private snapshots = new Set<string>();
  private latestGenerations = new Map<string, number>();
  private readonly ready = new Deferred<void>();
  private restarts = 0;
  private failed = false;
  private disposed = false;
  private fallback: InlineBackend;

  constructor(
    private graph: OGWorldGraph,
    private module: WebAssembly.Module,
    private workerURL: string | URL,
    private onFailure: (error: OGError) => void,
  ) {
    this.fallback = new InlineBackend(graph);
    this.boot();
  }

  get worker(): Worker | undefined {
    return this.live;
  }

  private boot(): void {
    try {
      const worker = new Worker(this.workerURL, { type: 'module' });
      this.live = worker;
      worker.onmessage = (event: MessageEvent<WorkerReply>) => { this.receive(event.data); };
      worker.onerror = () => { this.crash(); };
      worker.onmessageerror = () => { this.crash(); };
      worker.postMessage({ kind: 'init', request: 0, module: this.module } satisfies WorkerMessage);
    } catch {
      this.crash();
    }
  }

  private crash(): void {
    if (this.disposed || this.failed) return;
    this.live?.terminate();
    this.live = undefined;
    this.snapshots.clear();
    if (this.restarts > 0) {
      this.fail();
      return;
    }
    this.restarts++;
    this.drain();
    this.boot();
  }

  private fail(): void {
    this.failed = true;
    const error = new OGError('WorkerFailure', 'tessellation', 'tessellation worker failed');
    this.ready.reject(error);
    this.drain();
    queueMicrotask(() => { if (!this.disposed) this.onFailure(error); });
  }

  private drain(): void {
    const entries = [...this.pending.entries()];
    this.pending.clear();
    for (const [request, { value, resolve, reject }] of entries) {
      try {
        resolve(value ? { request, ok: true, buffers: this.fallback.compute(value) } : undefined);
      } catch (error) {
        reject(error);
      }
    }
  }

  private receive(reply: WorkerReply): void {
    if ('kind' in reply) {
      this.snapshots.delete(`${reply.shapeId}@${String(reply.revision)}`);
      return;
    }
    if (reply.request === 0) {
      if ('ok' in reply) this.ready.resolve();
      else this.crash();
      return;
    }
    if (reply.request === undefined) return;
    const pending = this.pending.get(reply.request);
    if (!pending) return;
    this.pending.delete(reply.request);
    if ('error' in reply) {
      pending.reject(new OGError(reply.error.code, 'tessellation', reply.error.message, reply.error.details));
      if (reply.error.code === 'KernelPanic') this.crash();
    } else pending.resolve(reply);
  }

  private async send(message: SendBody, transfer: Transferable[] = []): Promise<WorkerReply | undefined> {
    try {
      await this.ready.promise;
    } catch {
      return undefined;
    }
    const worker = this.live;
    if (this.failed || this.disposed || !worker) return undefined;
    const request = this.nextRequest++;
    const value = message.kind === 'tessellate' ? message : undefined;
    if (value) this.recordSend(value);
    return new Promise((resolve, reject) => {
      this.pending.set(request, { value, resolve, reject });
      worker.postMessage({ ...message, request }, transfer);
    });
  }

  private recordSend(value: TessellationRequest): void {
    const { shapeId, bucket, priority, generation } = value;
    this.sendLog.push({ kind: 'tessellate', shapeId, bucket, priority, generation });
    if (this.sendLog.length > SEND_LOG_LIMIT) this.sendLog.shift();
  }

  async ensureSnapshot(shapeId: string, revision: number): Promise<void> {
    const key = `${shapeId}@${String(revision)}`;
    if (this.failed || this.snapshots.has(key)) return;
    const bytes = call('tessellation', () => this.graph.snapshot(shapeId));
    const reply = await this.send({ kind: 'snapshot', shapeId, revision, bytes }, [bytes.buffer]);
    if (!reply) {
      if (this.live) await this.ensureSnapshot(shapeId, revision);
      return;
    }
    if (!('ok' in reply)) throw new OGError('WorkerFailure', 'snapshot', 'worker rejected snapshot');
    this.snapshots.add(key);
  }

  async request(value: TessellationRequest): Promise<DisplayBuffers> {
    if (this.failed) return this.fallback.compute(value);
    this.latestGenerations.set(value.shapeId, value.generation);
    await Promise.resolve();
    if (this.latestGenerations.get(value.shapeId) !== value.generation) {
      throw new OGError('Cancelled', 'tessellation', 'tessellation request superseded');
    }
    await this.ensureSnapshot(value.shapeId, value.revision);
    if (this.latestGenerations.get(value.shapeId) !== value.generation) {
      throw new OGError('Cancelled', 'tessellation', 'tessellation request superseded');
    }
    let reply = await this.send(tessellate(value));
    if (reply && 'missingSnapshot' in reply) {
      this.snapshots.delete(`${value.shapeId}@${String(value.revision)}`);
      await this.ensureSnapshot(value.shapeId, value.revision);
      reply = await this.send(tessellate(value));
    }
    return this.buffersFrom(value, reply);
  }

  private buffersFrom(value: TessellationRequest, reply: WorkerReply | undefined): DisplayBuffers {
    if (!reply) {
      if (this.failed) return this.fallback.compute(value);
      throw new OGError('WorkerFailure', 'tessellation', 'runtime disposed');
    }
    if ('superseded' in reply) throw new OGError('Cancelled', 'tessellation', 'tessellation request superseded');
    if ('buffers' in reply) return reply.buffers;
    throw new OGError('WorkerFailure', 'tessellation', 'worker returned no buffers');
  }

  cancelShape(shapeId: string, generation: number): void {
    this.live?.postMessage({ kind: 'cancel', shapeId, generation } satisfies WorkerMessage);
  }

  drop(shapeId: string, revision: number): void {
    this.snapshots.delete(`${shapeId}@${String(revision)}`);
    this.live?.postMessage({ kind: 'drop', shapeId, revision } satisfies WorkerMessage);
  }

  dispose(): void {
    this.disposed = true;
    this.live?.terminate();
    this.live = undefined;
    const error = new OGError('WorkerFailure', 'tessellation', 'runtime disposed');
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    this.snapshots.clear();
    this.ready.reject(error);
  }
}

function tessellate(value: TessellationRequest): Omit<TessellateMessage, 'request'> {
  const { shapeId, revision, bucket, priority, maxTriangles, generation } = value;
  return { kind: 'tessellate', shapeId, revision, bucket, priority, maxTriangles, generation };
}
