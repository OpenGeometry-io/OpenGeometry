import type { Body } from '../../bodies/body';
import type { Runtime } from '../../runtime/runtime-state';

export function shown(state: Runtime, body: Body): boolean {
  return body.visible || state.hostDrawn.get(body) === true;
}

export function setShown(state: Runtime, body: Body, value: boolean): void {
  if (state.hostDrawn.has(body)) state.hostDrawn.set(body, value);
  else body.visible = value;
}

export function handToHost(state: Runtime, body: Body, hostDrawn: boolean): void {
  const logical = state.hostDrawn.get(body);
  if (hostDrawn && logical === undefined) {
    state.hostDrawn.set(body, body.visible);
    body.visible = false;
  } else if (!hostDrawn && logical !== undefined) {
    state.hostDrawn.delete(body);
    body.visible = logical;
  }
}
