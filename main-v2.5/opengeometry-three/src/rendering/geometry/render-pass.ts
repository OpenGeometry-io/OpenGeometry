import type { Body } from '../../bodies/body.js';
import { currentRuntime, runtime } from '../../runtime/runtime-state.js';
import { ensureGeometry, flush } from './geometry-scheduler.js';

export function noteDisplayed(body: Body): void {
  const state = runtime();
  if (!state.renderPassActive) {
    state.renderPassActive = true;
    queueMicrotask(() => {
      if (currentRuntime() !== state) return;
      state.renderPassActive = false;
      if (state.readyVersion !== state.flushedReadyVersion) flush();
    });
  }
  state.displayed.add(body);
  flush();
  ensureGeometry(body);
}
