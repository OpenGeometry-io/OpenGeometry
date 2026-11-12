import type { Body } from '../../bodies/body';
import type { Runtime } from '../../runtime/runtime-state';
import { shown } from './host-drawn';
import { groupDisplayed, groupedBodies } from './host-groups';

export function isDisplayed(state: Runtime, body: Body): boolean {
  if (!shown(state, body)) return false;
  return (state.displayed.get(body) ?? -Infinity) >= state.pass - 1 || groupDisplayed(state, body);
}

export function displayedBodies(state: Runtime): Body[] {
  return [...new Set([...state.displayed.keys(), ...groupedBodies(state)])].filter((body) => isDisplayed(state, body));
}

export function stampedCount(state: Runtime): number {
  let count = 0;
  for (const body of state.displayed.keys()) if (isDisplayed(state, body)) count++;
  return count;
}

export function stampDisplayed(state: Runtime, body: Body): void {
  if (!isDisplayed(state, body)) wakeDisplayed(state, body);
  state.displayed.set(body, state.pass);
}

export function wakeDisplayed(state: Runtime, body: Body): void {
  const shapeId = body.lastInfo.shapeId;
  state.lod.stale = true;
  if (shapeId) delete state.lod.shapes.get(shapeId)?.overrides;
}
