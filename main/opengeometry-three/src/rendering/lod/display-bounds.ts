import type { Body } from '../../bodies/body';
import type { Bounds } from '../../dto/bounds';
import { call } from '../../kernel/kernel-session';
import type { Runtime } from '../../runtime/runtime-state';
import { decodeBounds } from '../../world-graph/codec';

export function displayBounds(state: Runtime, body: Body, revision: bigint): Bounds | null {
  const cached = state.displayBounds.get(body);
  if (cached?.revision === revision) return cached.bounds;
  const label = `${body.bodyType}.getBounds`;
  const bounds = decodeBounds(call(label, () => state.graph.bounds(body.ogId)), label);
  state.displayBounds.set(body, { revision, bounds });
  return bounds;
}
