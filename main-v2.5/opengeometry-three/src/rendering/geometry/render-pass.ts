import type { Body } from '../../bodies/body.js';
import { currentRuntime, runtime, type Runtime } from '../../runtime/runtime-state.js';
import { displayedBodies, evaluatePass, stampDisplayed } from '../lod/lod-controller.js';
import { ensureGeometry, flush } from './geometry-scheduler.js';

export function noteDisplayed(body: Body): void {
  const state = runtime();
  if (!state.renderPassActive) {
    state.renderPassActive = true;
    queueMicrotask(() => { endPass(state); });
  }
  stampDisplayed(state, body);
  flush();
  ensureGeometry(body);
}

function endPass(state: Runtime): void {
  if (currentRuntime() !== state) return;
  state.renderPassActive = false;
  state.pass++;
  if (state.readyVersion !== state.flushedReadyVersion) flush();
  if (evaluatePass(state, Date.now(), false)) refreshDisplayed(state);
}

export function refreshDisplayed(state: Runtime): void {
  for (const body of displayedBodies(state)) ensureGeometry(body);
}
