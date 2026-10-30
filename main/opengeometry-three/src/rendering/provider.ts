import type { DisplayBuffers } from '../dto/display-buffers';

export type TessellationRequest = {
  shapeId: string;
  revision: number;
  bucket: number;
  priority: number;
  maxTriangles: number;
  generation: number;
};

export type TessellationProvider = {
  readonly activeBackend: 'worker' | 'inline';
  ensureSnapshot(shapeId: string, revision: number): Promise<void>;
  request(value: TessellationRequest): Promise<DisplayBuffers>;
  cancelShape(shapeId: string, generation: number): void;
  drop(shapeId: string, revision: number): void;
  dispose(): void;
  compute?(value: TessellationRequest): DisplayBuffers;
};
