import type { DisplayBuffers } from '../dto/display-buffers.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import { call } from '../kernel/kernel-session.js';
import type { TessellationProvider, TessellationRequest } from './provider.js';

export class InlineBackend implements TessellationProvider {
  readonly activeBackend = 'inline' as const;

  constructor(private graph: OGWorldGraph) {}

  ensureSnapshot(): Promise<void> {
    return Promise.resolve();
  }

  compute(value: TessellationRequest): DisplayBuffers {
    const { shapeId, bucket, maxTriangles } = value;
    return call('tessellation', (): unknown => this.graph.buffers(shapeId, bucket, maxTriangles)) as DisplayBuffers;
  }

  async request(value: TessellationRequest): Promise<DisplayBuffers> {
    return this.compute(value);
  }

  cancelShape(): void {}
  drop(): void {}
  dispose(): void {}
}
