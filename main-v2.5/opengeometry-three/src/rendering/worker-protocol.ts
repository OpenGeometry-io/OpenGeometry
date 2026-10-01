import type { DisplayBuffers } from '../dto/display-buffers.js';
import type { TessellationRequest } from './provider.js';

export type WorkerError = { code: string; message: string; details?: unknown };

export type SnapshotMessage = {
  kind: 'snapshot';
  request: number;
  shapeId: string;
  revision: number;
  bytes: Uint8Array;
};

export type TessellateMessage = TessellationRequest & { kind: 'tessellate'; request: number };

export type WorkerMessage =
  | { kind: 'init'; request: 0; module: WebAssembly.Module }
  | SnapshotMessage
  | TessellateMessage
  | { kind: 'cancel'; shapeId: string; generation: number }
  | { kind: 'drop'; shapeId: string; revision: number };

export type WorkerReply =
  | { request: number; ok: true }
  | { request: number; ok: true; buffers: DisplayBuffers }
  | { request: number | undefined; error: WorkerError }
  | { request: number; superseded: true }
  | { request: number; missingSnapshot: true }
  | { kind: 'evicted'; shapeId: string; revision: number };
