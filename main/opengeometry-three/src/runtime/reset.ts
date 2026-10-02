import { createRuntime } from './create-runtime';
import { currentRuntime, setRuntime } from './runtime-state';

export function reset(): void {
  const state = currentRuntime();
  if (!state) return;
  for (const body of [...state.bodies.values()]) {
    body.visible = false;
    body.finalize();
  }
  state.provider.dispose();
  state.records.clear();
  for (const entry of [...state.surfacePool.values(), ...state.linePool.values()]) entry.material.dispose();
  state.surfacePool.clear();
  state.linePool.clear();
  state.graph.free();
  setRuntime(createRuntime(state.module, state.createOptions));
}
