import { runtime } from '../runtime/runtime-state.js';
import type { Body } from './body.js';

export function register(body: Body): void {
  runtime().bodies.set(body.ogId, body);
}

export function unregister(body: Body): void {
  const state = runtime();
  state.displayed.delete(body);
  state.bodies.delete(body.ogId);
  if (body.record) state.records.release(body.record);
  const info = body.lastInfo;
  if (info.shapeId && ![...state.bodies.values()].some((other) => other.lastInfo.shapeId === info.shapeId)) {
    state.records.purge(info.shapeId);
    state.provider.drop(info.shapeId, info.shapeRevision ?? 0);
  }
}
