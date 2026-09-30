import { currentRuntime, setRuntime } from './runtime-state.js';

export function reset(): void {
  const state = currentRuntime();
  if (!state) return;
  state.provider.dispose();
  state.records.clear();
  state.graph.free();
  setRuntime(undefined);
}
