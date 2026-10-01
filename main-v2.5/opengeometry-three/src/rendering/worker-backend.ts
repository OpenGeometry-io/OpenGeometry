import type { DisplayBuffers } from '../dto/display-buffers.js';
import { OGError } from '../errors.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import { call } from '../kernel/kernel-session.js';
import { InlineBackend } from './inline-backend.js';
import type { TessellationProvider, TessellationRequest } from './provider.js';
import type { WorkerReply } from './worker-protocol.js';

export class WorkerBackend implements TessellationProvider {
  activeBackend: 'worker' | 'inline' = 'worker';
  private worker: Worker | undefined;
  private nextRequest = 1;
  private pending = new Map<number, { resolve: (value: WorkerReply) => void; reject: (error: OGError) => void }>();
  private snapshots = new Set<string>();
  private latestGenerations = new Map<string, number>();
  private ready: Promise<void>;
  private readyResolve!: () => void;
  private readyReject!: (error: OGError) => void;
  private restarts = 0;
  private disposed = false;
  private fallback: InlineBackend;

  constructor(
    private graph: OGWorldGraph,
    private module: WebAssembly.Module,
    private workerURL: string | URL,
    private onFailure: (error: OGError) => void,
  ) {
    this.fallback = new InlineBackend(graph);
    this.ready = new Promise((resolve, reject) => { this.readyResolve = resolve; this.readyReject = reject; });
    this.boot();
  }

  private boot(): void {
    try {
      const worker = new Worker(this.workerURL, { type: 'module' });
      this.worker = worker;
      worker.onmessage = (event: MessageEvent<WorkerReply>) => { this.receive(event.data); };
      worker.onerror = () => { this.crash(); };
      worker.onmessageerror = () => { this.crash(); };
      worker.postMessage({ kind: 'init', request: 0, module: this.module });
    } catch {
      this.crash();
    }
  }

  private crash(): void {
    if (this.disposed) return;
    this.worker?.terminate();
    this.worker = undefined;
    this.snapshots.clear();
    const error = new OGError('WorkerFailure', 'tessellation', 'tessellation worker failed');
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    if (this.restarts < 1) {
      this.restarts++;
      this.ready = new Promise((resolve, reject) => { this.readyResolve = resolve; this.readyReject = reject; });
      this.boot();
    } else {
      this.activeBackend = 'inline';
      this.readyReject(error);
      this.onFailure(error);
    }
  }

  private receive(reply: WorkerReply): void {
    if (reply.request === 0) {
      if (reply.ok) this.readyResolve();
      else this.crash();
      return;
    }
    if (reply.kind === 'evicted' && reply.shapeId !== undefined) {
      this.snapshots.delete(`${reply.shapeId}@${String(reply.revision)}`);
      return;
    }
    if (reply.request === undefined) return;
    const pending = this.pending.get(reply.request);
    if (!pending) return;
    this.pending.delete(reply.request);
    if (reply.error) {
      pending.reject(new OGError(reply.error.code, 'tessellation', reply.error.message, reply.error.details));
      if (reply.error.code === 'KernelPanic') this.crash();
    } else pending.resolve(reply);
  }

  private async send(message: Record<string, unknown>, transfer: Transferable[] = []): Promise<WorkerReply> {
    await this.ready;
    const request = this.nextRequest++;
    return new Promise((resolve, reject) => {
      this.pending.set(request, { resolve, reject });
      this.worker?.postMessage({ ...message, request }, transfer);
    });
  }

  async ensureSnapshot(shapeId: string, revision: number): Promise<void> {
    if (this.activeBackend === 'inline') return;
    const key = `${shapeId}@${String(revision)}`;
    if (this.snapshots.has(key)) return;
    const bytes = call('tessellation', () => this.graph.snapshot(shapeId));
    const reply = await this.send({ kind: 'snapshot', shapeId, revision, bytes }, [bytes.buffer]);
    if (!reply.ok) throw new OGError('WorkerFailure', 'snapshot', 'worker rejected snapshot');
    this.snapshots.add(key);
  }

  async request(value: TessellationRequest): Promise<DisplayBuffers> {
    if (this.activeBackend === 'inline') return this.fallback.compute(value);
    this.latestGenerations.set(value.shapeId, value.generation);
    await Promise.resolve();
    if (this.latestGenerations.get(value.shapeId) !== value.generation) {
      throw new OGError('Cancelled', 'tessellation', 'tessellation request superseded');
    }
    await this.ensureSnapshot(value.shapeId, value.revision);
    if (this.latestGenerations.get(value.shapeId) !== value.generation) {
      throw new OGError('Cancelled', 'tessellation', 'tessellation request superseded');
    }
    let reply = await this.send({ kind: 'tessellate', ...value });
    if (reply.missingSnapshot) {
      this.snapshots.delete(`${value.shapeId}@${String(value.revision)}`);
      await this.ensureSnapshot(value.shapeId, value.revision);
      reply = await this.send({ kind: 'tessellate', ...value });
    }
    if (reply.superseded) throw new OGError('Cancelled', 'tessellation', 'tessellation request superseded');
    if (!reply.ok || !reply.buffers) throw new OGError('WorkerFailure', 'tessellation', 'worker returned no buffers');
    return reply.buffers;
  }

  cancelShape(shapeId: string, generation: number): void {
    this.worker?.postMessage({ kind: 'cancel', shapeId, generation });
  }

  drop(shapeId: string, revision: number): void {
    this.snapshots.delete(`${shapeId}@${String(revision)}`);
    this.worker?.postMessage({ kind: 'drop', shapeId, revision });
  }

  dispose(): void {
    this.disposed = true;
    this.worker?.terminate();
    this.worker = undefined;
    for (const pending of this.pending.values()) {
      pending.reject(new OGError('WorkerFailure', 'tessellation', 'runtime disposed'));
    }
    this.pending.clear();
    this.snapshots.clear();
  }

  async debugStats(): Promise<Record<string, number>> {
    return (await this.send({ kind: 'stats' })).stats ?? {};
  }
}
