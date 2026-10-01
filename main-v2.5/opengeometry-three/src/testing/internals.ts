import { runtime } from '../runtime/runtime-state.js';

export { register, unregister } from '../bodies/body-registry.js';
export { ensureGeometry, flush, wanted } from '../rendering/geometry/geometry-scheduler.js';
export { noteDisplayed } from '../rendering/geometry/render-pass.js';
export { wantedBucket } from '../rendering/lod/deflection.js';
export { emit } from '../runtime/event-bus.js';
export { currentRuntime, runtime } from '../runtime/runtime-state.js';
export { worldGraph as graph } from '../world-graph/world-graph-client.js';

export function activeBackend(): 'worker' | 'inline' { return runtime().provider.activeBackend; }

export function flushCount(): number { return runtime().flushes; }
