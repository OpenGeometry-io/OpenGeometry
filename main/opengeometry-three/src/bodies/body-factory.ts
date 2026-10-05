import { call } from '../kernel/kernel-session';
import { encode, operationParams, polylinePoints } from '../world-graph/codec';
import { creation, worldGraph } from '../world-graph/world-graph-client';
import type { BodyOptions } from './body-options';

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
  const label = `${bodyType}.constructor`;
  if (kind === 'Polyline') {
    const points = polylinePoints(params, label);
    const closed = Boolean(params['closed']);
    return creation(call(label, () => graph.createPolyline(points, closed, encode(settings))), label);
  }
  const operation = kind === 'Extrude' || kind === 'Sweep';
  return creation(call(label, () => operation
    ? graph.createOperation(encode(operationParams(kind, params, label)), encode(settings))
    : graph.createPrimitive(encode({ kind, ...params }), encode(settings))), label);
}

function existingNodeId(params: Record<string, unknown>): string | undefined {
  const ogId = params['ogId'];
  return params['existing'] === existingNodeId && typeof ogId === 'string' ? ogId : undefined;
}
