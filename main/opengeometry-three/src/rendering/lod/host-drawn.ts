import type { Body } from '../../bodies/body';
import type { Runtime } from '../../runtime/runtime-state';
import { shownChanged } from './host-groups';

export function shown(state: Runtime, body: Body): boolean {
  return body.visible || state.hostDrawn.get(body) === true;
}

export function setShown(state: Runtime, body: Body, value: boolean): void {
  const before = shown(state, body);
  if (state.hostDrawn.has(body)) state.hostDrawn.set(body, value);
  else body.visible = value;
  if (shown(state, body) !== before) shownChanged(state, body);
}

export function handToHost(state: Runtime, body: Body, hostDrawn: boolean): void {
  const handed = hostDrawn && !state.hostDrawn.has(body);
  const before = shown(state, body);
  shareDrawing(state, body, hostDrawn);
  if (handed ? shown(state, body) : shown(state, body) !== before) shownChanged(state, body);
}

function shareDrawing(state: Runtime, body: Body, hostDrawn: boolean): void {
  const logical = state.hostDrawn.get(body);
  if (hostDrawn && logical === undefined) {
    state.hostDrawn.set(body, body.visible);
    body.visible = false;
  } else if (!hostDrawn && logical !== undefined) {
    state.hostDrawn.delete(body);
    body.visible = logical;
  }
}
