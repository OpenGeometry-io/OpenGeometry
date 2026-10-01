import type { DisplayBuffers } from '../../dto/display-buffers.js';
import { OGError } from '../../errors.js';

const SCALARS = ['revision', 'bucket', 'achievedDeflection', 'triangles'] as const;

export function validateDisplayBuffers(value: unknown): DisplayBuffers {
  if (!isDisplayBuffers(value) || !hasConsistentLengths(value)) {
    throw new OGError('WorkerFailure', 'tessellation', 'invalid display buffer structure');
  }
  return value;
}

function isDisplayBuffers(value: unknown): value is DisplayBuffers {
  if (typeof value !== 'object' || value === null) return false;
  const field = (key: string): unknown => Reflect.get(value, key);
  return field('positions') instanceof Float32Array && field('normals') instanceof Float32Array
    && field('indices') instanceof Uint32Array && field('faceRanges') instanceof Uint32Array
    && field('outline') instanceof Float32Array && field('edgeIds') instanceof Uint32Array
    && field('origin') instanceof Float64Array && SCALARS.every((key) => typeof field(key) === 'number');
}

function hasConsistentLengths(buffers: DisplayBuffers): boolean {
  return buffers.origin.length === 3 && buffers.positions.length % 3 === 0
    && buffers.normals.length === buffers.positions.length
    && buffers.indices.length % 3 === 0 && buffers.indices.length <= 6_000_000
    && buffers.outline.length % 6 === 0 && buffers.outline.length <= 12_000_000
    && buffers.edgeIds.length === buffers.outline.length / 6 && buffers.faceRanges.length % 3 === 0;
}
