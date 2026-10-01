import { call } from '../kernel/kernel-session.js';
import { encode, operationParams } from '../world-graph/codec.js';
import { creation, worldGraph } from '../world-graph/world-graph-client.js';
import type { BodyOptions } from './body-options.js';

export function existingParams(ogId: string): Record<string, unknown> {
  return { existing: existingNodeId, ogId };
}

export function createBody(
  kind: string, params: Record<string, unknown>, options: BodyOptions, bodyType: 'Wire' | 'Solid',
): string {
  const existing = existingNodeId(params);
  if (existing !== undefined) return existing;
  const graph = worldGraph();
  const settings = { ogId: options.ogId, parent: options.parent?.ogId, plane: options.plane, bodyType };
  if (kind === 'Polyline') {
    const points = params['points'] as [number, number, number][];
    return creation(call('Wire.constructor', () => graph.createPolyline(
      new Float64Array(points.flat()), Boolean(params['closed']), encode(settings),
    )));
  }
  const operation = kind === 'Extrude' || kind === 'Sweep';
  return creation(call('Body.constructor', () => operation
    ? graph.createOperation(encode(operationParams(kind, params)), encode(settings))
    : graph.createPrimitive(encode({ kind, ...params }), encode(settings))));
}

function existingNodeId(params: Record<string, unknown>): string | undefined {
  const ogId = params['ogId'];
  return params['existing'] === existingNodeId && typeof ogId === 'string' ? ogId : undefined;
}
