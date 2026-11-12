import type { Body } from '../../bodies/body';
import { OGError } from '../../errors';
import { currentRuntime, runtime, type Runtime } from '../../runtime/runtime-state';
import { handToHost, shown } from '../lod/host-drawn';
import { displayedBodies, isDisplayed, stampDisplayed, wakeDisplayed } from '../lod/displayed';
import { takeWaiting } from '../lod/host-groups';
import { evaluatePass } from '../lod/lod-controller';
import { checkedBodies, ensureGeometry, flush } from './geometry-scheduler';

export function noteDisplayed(body: Body): void {
  const state = runtime();
  const starting = beginPass(state);
  stampDisplayed(state, body);
  state.lod.walked.set(body, state.pass);
  state.lod.drawn.delete(body);
  if (starting || state.graphMoved) flush();
  ensureGeometry(body);
}

export function noteDrawn(body: Body): void {
  const state = runtime();
  if (!shown(state, body) || (state.renderPassActive && state.lod.walked.get(body) === state.pass)) return;
  beginPass(state);
  stampDisplayed(state, body);
  state.lod.drawn.add(body);
}

export function markDisplayed(bodies: readonly Body[]): void {
  const state = runtime();
  for (const body of checkedBodies(state, bodies, 'OpenGeometry.markDisplayed')) noteDrawn(body);
}

export function setHostDrawn(bodies: readonly Body[], hostDrawn: boolean): void {
  const state = runtime();
  const label = 'OpenGeometry.setHostDrawn';
  const flag: unknown = hostDrawn;
  if (typeof flag !== 'boolean') throw new OGError('InvalidParameter', label, 'hostDrawn must be a boolean');
  for (const body of checkedBodies(state, bodies, label, !hostDrawn)) handToHost(state, body, hostDrawn);
}

export function beginPass(state: Runtime): boolean {
  if (state.renderPassActive) return false;
  state.renderPassActive = true;
  queueMicrotask(() => { endPass(state); });
  return true;
}

function endPass(state: Runtime): void {
  if (currentRuntime() !== state) return;
  const drawn = new Set(state.lod.drawn);
  state.lod.drawn.clear();
  try {
    installDrawn(state, drawn);
  } finally {
    state.renderPassActive = false;
    state.pass++;
  }
  if (state.readyShapes.size > 0) flush();
  if (evaluatePass(state, Date.now(), false)) refreshDisplayed(state);
}

function installDrawn(state: Runtime, drawn: ReadonlySet<Body>): void {
  const groups = state.groups;
  if (drawn.size === 0 && !groups.drawn && groups.pending.size === 0 && groups.woken.size === 0) return;
  flush();
  for (const body of takeWaiting(state)) {
    if (drawn.has(body) || !isDisplayed(state, body)) continue;
    wakeDisplayed(state, body);
    ensureGeometry(body);
  }
  for (const body of drawn) if (isDisplayed(state, body)) ensureGeometry(body);
}

export function refreshDisplayed(state: Runtime): void {
  for (const body of displayedBodies(state)) ensureGeometry(body);
}
