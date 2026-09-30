import type { DisplayBuffers } from '../dto/display-buffers.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import type { TessellationProvider, TessellationRequest } from './provider.js';

export class InlineBackend implements TessellationProvider {
  readonly activeBackend = 'inline' as const;

  constructor(private graph: OGWorldGraph) {}

  ensureSnapshot(): Promise<void> {
    return Promise.resolve();
  }

  compute(value: TessellationRequest): DisplayBuffers {
    return this.graph.buffers(value.shapeId, value.bucket, value.maxTriangles) as DisplayBuffers;
  }

  async request(value: TessellationRequest): Promise<DisplayBuffers> {
    return this.compute(value);
  }

  cancelShape(): void {}
  drop(): void {}
  dispose(): void {}
}
