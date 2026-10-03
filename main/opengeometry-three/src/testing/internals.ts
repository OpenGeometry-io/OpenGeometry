import { OGError } from '../errors';
import { forceNextBucket } from '../rendering/lod/lod-controller';
import { WorkerBackend, type WorkerSend } from '../rendering/worker-backend';
import { runtime } from '../runtime/runtime-state';

export { register, unregister } from '../bodies/body-registry';
export { ensureGeometry, flush, wanted } from '../rendering/geometry/geometry-scheduler';
export { noteDisplayed } from '../rendering/geometry/render-pass';
export { wantedBucket } from '../rendering/lod/deflection';
export { parseWorkerMessage } from '../rendering/worker/message-guards';
export { emit } from '../runtime/event-bus';
export { currentRuntime, runtime } from '../runtime/runtime-state';
export { worldGraph as graph } from '../world-graph/world-graph-client';

export function activeBackend(): 'worker' | 'inline' { return runtime().provider.activeBackend; }

export function flushCount(): number { return runtime().flushes; }

export function pinWantedBucket(shapeId: string, bucket: number): void { forceNextBucket(runtime(), shapeId, bucket); }

export function workerSendLog(): WorkerSend[] {
  const provider = runtime().provider;
  return provider instanceof WorkerBackend ? [...provider.sendLog] : [];
}

export function postToWorker(message: unknown, options: { awaitCrash?: boolean } = {}): Promise<void> {
  const provider = runtime().provider;
  const worker = provider instanceof WorkerBackend ? provider.worker : undefined;
  if (!worker) return Promise.reject(new OGError('WorkerFailure', 'testing', 'no live worker'));
  const crashed = options.awaitCrash
    ? new Promise<void>((resolve) => { worker.addEventListener('error', () => { resolve(); }, { once: true }); })
    : Promise.resolve();
  worker.postMessage(message);
  return crashed;
}
