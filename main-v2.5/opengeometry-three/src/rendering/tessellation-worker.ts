import { initSync, OGTessellator } from '../kernel/kernel-loader.js';
import type { DisplayBuffers } from '../dto/display-buffers.js';

type Job = {
  request: number;
  shapeId: string;
  revision: number;
  bucket: number;
  maxTriangles: number;
  generation: number;
  priority: number;
};
type Snapshot = { slot: number; bytes: number; lastUse: number };
type WorkerError = { code: string; message: string; details?: unknown };

let tessellator: OGTessellator | undefined;
let totalBytes = 0;
let clock = 0;
let scheduled = false;
const SNAPSHOTS = new Map<string, Snapshot>();
const JOBS = new Map<string, Job>();
const JOBS_STARTED = new Map<string, number>();
const CHANNEL = new MessageChannel();

function error(value: unknown): WorkerError {
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value) as { code: string; message: string; details?: unknown };
      if (parsed.code && parsed.message) return parsed;
    } catch {
      return { code: 'WorkerFailure', message: value, details: {} };
    }
  }
  return { code: 'WorkerFailure', message: String(value), details: {} };
}

function evict(except: string): void {
  while (totalBytes > 128 * 1024 * 1024) {
    const oldest = [...SNAPSHOTS.entries()]
      .filter(([key]) => key !== except)
      .sort((a, b) => a[1].lastUse - b[1].lastUse)[0];
    if (!oldest) break;
    const [key, value] = oldest;
    tessellator?.drop(value.slot);
    SNAPSHOTS.delete(key);
    totalBytes -= value.bytes;
    const at = key.lastIndexOf('@');
    self.postMessage({ kind: 'evicted', shapeId: key.slice(0, at), revision: Number(key.slice(at + 1)) });
  }
}

function schedule(): void {
  if (scheduled || JOBS.size === 0) return;
  scheduled = true;
  CHANNEL.port2.postMessage(null);
}

CHANNEL.port1.onmessage = () => {
  scheduled = false;
  const next = [...JOBS.values()].sort((a, b) => a.priority - b.priority)[0];
  if (!next) return;
  JOBS.delete(next.shapeId);
  JOBS_STARTED.set(next.shapeId, (JOBS_STARTED.get(next.shapeId) ?? 0) + 1);
  const snapshot = SNAPSHOTS.get(`${next.shapeId}@${String(next.revision)}`);
  if (!snapshot || !tessellator) {
    self.postMessage({ request: next.request, missingSnapshot: true });
  } else {
    snapshot.lastUse = ++clock;
    try {
      const buffers = tessellator.buffers(snapshot.slot, next.bucket, next.maxTriangles) as DisplayBuffers;
      self.postMessage({ request: next.request, ok: true, buffers }, [
        buffers.positions.buffer, buffers.normals.buffer, buffers.indices.buffer,
        buffers.faceRanges.buffer, buffers.outline.buffer, buffers.edgeIds.buffer, buffers.origin.buffer,
      ]);
    } catch (cause) {
      self.postMessage({ request: next.request, error: error(cause) });
    }
  }
  schedule();
};

self.onmessage = (event: MessageEvent<Record<string, unknown>>) => {
  const message = event.data;
  const kind = message['kind'];
  const request = Number(message['request']);
  try {
    if (kind === 'init') {
      initSync({ module: message['module'] as WebAssembly.Module });
      tessellator = new OGTessellator();
      self.postMessage({ request, ok: true });
    } else if (kind === 'snapshot') {
      const shapeId = String(message['shapeId']);
      const revision = Number(message['revision']);
      const bytes = message['bytes'] as Uint8Array;
      const key = `${shapeId}@${String(revision)}`;
      const old = SNAPSHOTS.get(key);
      if (old) {
        tessellator?.drop(old.slot);
        totalBytes -= old.bytes;
      }
      const slot = tessellator!.load(bytes);
      SNAPSHOTS.set(key, { slot, bytes: bytes.byteLength, lastUse: ++clock });
      totalBytes += bytes.byteLength;
      evict(key);
      self.postMessage({ request, ok: true });
    } else if (kind === 'tessellate') {
      const job = message as unknown as Job;
      const previous = JOBS.get(job.shapeId);
      if (previous) self.postMessage({ request: previous.request, superseded: true });
      JOBS.set(job.shapeId, job);
      schedule();
    } else if (kind === 'cancel') {
      const shapeId = String(message['shapeId']);
      const previous = JOBS.get(shapeId);
      if (previous && previous.generation <= Number(message['generation'])) {
        JOBS.delete(shapeId);
        self.postMessage({ request: previous.request, superseded: true });
      }
    } else if (kind === 'drop') {
      const key = `${String(message['shapeId'])}@${String(message['revision'])}`;
      const old = SNAPSHOTS.get(key);
      if (old) {
        tessellator?.drop(old.slot);
        SNAPSHOTS.delete(key);
        totalBytes -= old.bytes;
      }
    } else if (kind === 'stats') {
      self.postMessage({ request, ok: true, stats: Object.fromEntries(JOBS_STARTED) });
    }
  } catch (cause) {
    self.postMessage({ request, error: error(cause) });
  }
};
