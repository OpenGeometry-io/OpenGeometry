import { bodyKey, reindexShape, releaseShapeIfEmpty, runtime } from '../runtime/runtime-state.js';
import type { Body } from './body.js';

export function register(body: Body): void {
  const state = runtime();
  state.bodies.set(bodyKey(body.handle, body.generation), body);
  state.pickTargets.set(body.surface, body).set(body.outline, body);
  reindexShape(state, body, null);
}

export function unregister(body: Body): void {
  const state = runtime();
  state.displayed.delete(body);
  state.limbo.delete(body);
  state.bodies.delete(bodyKey(body.handle, body.generation));
  state.pickTargets.delete(body.surface);
  state.pickTargets.delete(body.outline);
  if (body.record) state.records.release(body.record);
  const info = body.lastInfo;
  if (!info.shapeId) return;
  state.byShape.get(info.shapeId)?.delete(body);
  releaseShapeIfEmpty(state, info.shapeId, info.shapeRevision ?? 0);
}
