import type { DisplayBuffers } from '../dto/display-buffers';
import type { OGWorldGraph } from '../kernel/kernel-loader';
import { call } from '../kernel/kernel-session';
import type { TessellationProvider, TessellationRequest } from './provider';
import { validateDisplayBuffers } from './records/validate-display-buffers';

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
