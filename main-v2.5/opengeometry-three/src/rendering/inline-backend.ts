import type { DisplayBuffers } from '../dto/display-buffers.js';
import type { OGWorldGraph } from '../kernel/kernel-loader.js';
import { call } from '../kernel/kernel-session.js';
import type { TessellationProvider, TessellationRequest } from './provider.js';
import { validateDisplayBuffers } from './records/validate-display-buffers.js';

export class InlineBackend implements TessellationProvider {
  readonly activeBackend = 'inline' as const;

  constructor(private graph: OGWorldGraph) {}

  ensureSnapshot(): Promise<void> {
    return Promise.resolve();
  }

  compute(value: TessellationRequest): DisplayBuffers {
    const { shapeId, bucket, maxTriangles } = value;
    const buffers = call('tessellation', (): unknown => this.graph.buffers(shapeId, bucket, maxTriangles));
    return validateDisplayBuffers(buffers);
  }

  request(value: TessellationRequest): Promise<DisplayBuffers> {
    return new Promise((resolve) => { resolve(this.compute(value)); });
  }

  cancelShape(): void {}
  drop(): void {}
  dispose(): void {}
}
