/**
 * Version string exposed by the Three.js adapter at runtime.
 */
export const OPEN_GEOMETRY_THREE_VERSION = "2.0.3";

/**
 * Options accepted when initializing the OpenGeometry runtime.
 * - `wasmURL`: Optional URL to the OpenGeometry wasm binary. If not provided, it defaults to a relative path.
 * - `debugMeshes`: Optional flag to enable debug meshes for visualizing geometry. Defaults to `false`.
 */
export interface OpenGeometryOptions {
  wasmURL?: string;
  debugMeshes?: boolean;
}
