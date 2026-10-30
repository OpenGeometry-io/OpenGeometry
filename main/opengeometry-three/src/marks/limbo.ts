import type { Body } from '../bodies/body';
import { call } from '../kernel/kernel-session';
import { bodyKey, reindexShape, runtime, type Runtime } from '../runtime/runtime-state';
import { node, worldGraph } from '../world-graph/world-graph-client';

export function enterLimbo(body: Body): void {
  if (body.inLimbo) return;
  const state = runtime();
  const parent = body.parent;
  hide(state, body);
  body.inLimbo = true;
  if (state.marks.size === 0) {
    body.finalize();
    return;
  }
  state.limbo.set(body, { watermark: Math.max(...state.marks.keys()), parent });
}

export function finalizeSettled(state: Runtime): void {
  const oldest = Math.min(...state.marks.keys());
  for (const [body, entry] of [...state.limbo]) if (entry.watermark < oldest) body.finalize();
}

export function revive(body: Body): void {
  const state = runtime();
  const entry = state.limbo.get(body);
  if (!entry) return;
  try {
    call('OGMark.rollback', () => worldGraph().nodeByHandle(body.handle, body.generation));
    state.limbo.delete(body);
    body.inLimbo = false;
    body.visible = true;
    entry.parent?.add(body);
    const previous = body.lastInfo.shapeId;
    body.lastInfo = node(body.ogId, 'OGMark.rollback');
    reindexShape(state, body, previous);
  } catch { body.visible = false; }
}

export function finalizeNow(body: Body): void {
  if (!body.inLimbo) hide(runtime(), body);
  body.finalize();
}

function hide(state: Runtime, body: Body): void {
  body.visible = false;
  state.displayed.delete(body);
  const parent = body.parent;
  if (state.flushing || state.renderPassActive) {
    queueMicrotask(() => { if (!revived(state, body)) parent?.remove(body); });
  } else parent?.remove(body);
}

function revived(state: Runtime, body: Body): boolean {
  return !body.inLimbo && state.bodies.get(bodyKey(body.handle, body.generation)) === body;
}
