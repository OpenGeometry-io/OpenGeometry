import { parseKernelError } from '../kernel/kernel-errors.js';
import { initSync, OGTessellator, takePanicMessage } from '../kernel/kernel-loader.js';
import { validateDisplayBuffers } from './records/validate-display-buffers.js';
import { JobQueue } from './worker/job-queue.js';
import { parseWorkerMessage } from './worker/message-guards.js';
import { SnapshotCache } from './worker/snapshot-cache.js';
import type { TessellateMessage, WorkerError, WorkerMessage, WorkerReply } from './worker-protocol.js';

let tessellator: OGTessellator | undefined;
let scheduled = false;
const SNAPSHOTS = new SnapshotCache();
const JOBS = new JobQueue();
const CHANNEL = new MessageChannel();

function error(value: unknown): WorkerError {
  if (value instanceof WebAssembly.RuntimeError) {
    return { code: 'KernelPanic', message: takePanicMessage() ?? value.message, details: {} };
  }
  const parsed = typeof value === 'string' ? parseKernelError(value) : undefined;
  return parsed ?? { code: 'WorkerFailure', message: String(value), details: {} };
}

function reply(message: WorkerReply, transfer: Transferable[] = []): void {
  self.postMessage(message, transfer);
}

function postEvicted(key: string): void {
  const at = key.lastIndexOf('@');
  reply({ kind: 'evicted', shapeId: key.slice(0, at), revision: Number(key.slice(at + 1)) });
}

function answerSuperseded(request: number | undefined): void {
  if (request !== undefined) reply({ request, superseded: true });
}

function schedule(): void {
  if (scheduled || JOBS.size === 0) return;
  scheduled = true;
  CHANNEL.port2.postMessage(null);
}

function run(job: TessellateMessage): void {
  const slot = SNAPSHOTS.touch(`${job.shapeId}@${String(job.revision)}`);
  if (slot === undefined || !tessellator) {
    reply({ request: job.request, missingSnapshot: true });
    return;
  }
  try {
    const buffers = validateDisplayBuffers(tessellator.buffers(slot, job.bucket, job.maxTriangles));
    reply({ request: job.request, ok: true, buffers }, [
      buffers.positions.buffer, buffers.normals.buffer, buffers.indices.buffer,
      buffers.faceRanges.buffer, buffers.outline.buffer, buffers.edgeIds.buffer, buffers.origin.buffer,
    ]);
  } catch (cause) {
    reply({ request: job.request, error: error(cause) });
  }
}

CHANNEL.port1.onmessage = () => {
  scheduled = false;
  const next = JOBS.next();
  if (!next) return;
  run(next);
  schedule();
};

function init(module: WebAssembly.Module): void {
  initSync({ module });
  tessellator = new OGTessellator();
  reply({ request: 0, ok: true });
}

function handle(message: Exclude<WorkerMessage, { kind: 'init' }>, loaded: OGTessellator): void {
  switch (message.kind) {
    case 'snapshot': {
      const key = `${message.shapeId}@${String(message.revision)}`;
      SNAPSHOTS.load(key, message.bytes, loaded);
      for (const evicted of SNAPSHOTS.evict(key, loaded)) postEvicted(evicted);
      reply({ request: message.request, ok: true });
      break;
    }
    case 'tessellate':
      answerSuperseded(JOBS.push(message));
      schedule();
      break;
    case 'cancel':
      answerSuperseded(JOBS.cancel(message.shapeId, message.generation));
      break;
    case 'drop':
      SNAPSHOTS.drop(`${message.shapeId}@${String(message.revision)}`, loaded);
      break;
  }
}

self.onmessage = (event: MessageEvent<unknown>) => {
  const message = parseWorkerMessage(event.data);
  const request = 'request' in message ? message.request : undefined;
  try {
    if ('error' in message) reply(message);
    else if (message.kind === 'init') init(message.module);
    else if (tessellator) handle(message, tessellator);
    else reply({ request, error: { code: 'WorkerFailure', message: 'tessellation worker is not initialised' } });
  } catch (cause) {
    reply({ request, error: error(cause) });
  }
};
