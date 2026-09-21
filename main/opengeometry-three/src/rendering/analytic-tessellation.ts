export interface AnalyticTessellationData {
  positions: Float64Array;
  normals: Float32Array;
  indices: Uint32Array;
  triangleFaceIds: Uint32Array;
  outlinePositions: Float64Array;
  outlineEdgeIds: Uint32Array;
  revision: bigint;
  achievedDeflection: number;
}

export const GEOMETRY_DEFLECTION_SHARE = 0.75;

export interface AnalyticWorkerOptions {
  workerURL: string | URL;
  kernelURL: string | URL;
  wasmURL: string | URL;
}

interface Pending {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  handle?: string;
  generation?: number;
}

export class AnalyticTessellationWorker {
  private readonly worker: Worker;
  private readonly pending = new Map<number, Pending>();
  private readonly generations = new Map<string, number>();
  private readonly ready: Promise<unknown>;
  private serial = 0;
  private closed = false;

  constructor(options: AnalyticWorkerOptions) {
    const base = document.baseURI;
    this.worker = new Worker(new URL(options.workerURL, base), { type: "module" });
    this.worker.onmessage = (event: MessageEvent) => {
      const message = event.data;
      const pending = this.pending.get(message?.request);
      if (!pending) return;
      this.pending.delete(message.request);
      if (pending.handle && pending.generation !== this.generations.get(pending.handle)) {
        pending.reject(new DOMException("Superseded tessellation request", "AbortError"));
      } else if (message.error) {
        pending.reject(new Error(typeof message.error === "string" ? message.error : JSON.stringify(message.error)));
      } else {
        pending.resolve(message.result);
      }
    };
    this.worker.onerror = (event) => this.fail(new Error(event.message || "Analytic worker failed"));
    this.worker.onmessageerror = () => this.fail(new Error("Analytic worker response could not be decoded"));
    this.ready = this.send({
      kind: "init", kernelURL: new URL(options.kernelURL, base).href, wasmURL: new URL(options.wasmURL, base).href,
    });
    // Initialization errors remain observable through every public operation.
    void this.ready.catch(() => undefined);
  }

  async setModel(handle: string, serialized: string): Promise<void> {
    await this.ready;
    if (!handle || handle.length > 1024) throw new Error("Worker model handle must contain 1–1024 characters");
    this.cancel(handle);
    await this.send({ kind: "model", handle, serialized });
  }

  async tessellate(handle: string, deflection: number): Promise<AnalyticTessellationData> {
    await this.ready;
    if (!Number.isFinite(deflection) || deflection <= 0) throw new Error("Deflection must be finite and positive");
    const generation = this.cancel(handle);
    return await this.send({ kind: "tessellate", handle, generation, deflection }, handle, generation) as AnalyticTessellationData;
  }

  cancel(handle: string): number {
    const generation = (this.generations.get(handle) ?? 0) + 1;
    this.generations.set(handle, generation);
    return generation;
  }

  async removeModel(handle: string): Promise<void> {
    this.cancel(handle);
    await this.ready;
    await this.send({ kind: "remove", handle });
    this.generations.delete(handle);
  }

  dispose(): void {
    this.fail(new DOMException("Analytic worker disposed", "AbortError"));
  }

  private send(message: Record<string, unknown>, handle?: string, generation?: number): Promise<unknown> {
    if (this.closed) return Promise.reject(new Error("Analytic worker is closed"));
    if (this.pending.size >= 128) return Promise.reject(new Error("Analytic worker request limit exceeded; previous mesh retained"));
    const request = ++this.serial;
    return new Promise((resolve, reject) => {
      this.pending.set(request, { resolve, reject, handle, generation });
      try { this.worker.postMessage({ ...message, request }); }
      catch (error) {
        this.pending.delete(request);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  private fail(error: Error): void {
    if (this.closed) return;
    this.closed = true;
    this.worker.terminate();
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    this.generations.clear();
  }
}
