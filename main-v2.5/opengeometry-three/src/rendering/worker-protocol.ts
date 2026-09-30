import type { DisplayBuffers } from '../dto/display-buffers.js';

export type WorkerReply = {
  request?: number;
  ok?: boolean;
  superseded?: boolean;
  missingSnapshot?: boolean;
  error?: { code: string; message: string; details?: unknown };
  kind?: string;
  shapeId?: string;
  revision?: number;
  buffers?: DisplayBuffers;
  stats?: Record<string, number>;
};
