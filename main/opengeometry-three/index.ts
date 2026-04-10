/**
 * Public OpenGeometry JavaScript entrypoint.
 */
import init, {
  OGSceneManager,
  Vector3,
} from "../opengeometry/pkg/opengeometry";
import { SpotLabel } from "./src/markup/spotMarker";
import { OPEN_GEOMETRY_THREE_VERSION, OpenGeometryOptions } from "./src/base-types";

export type OUTLINE_TYPE = "front" | "side" | "top";

/**
 * Binary STL export payload returned by export helpers.
 */
export interface OGStlExportResult {
  bytes: Uint8Array;
  reportJson: string;
}

/**
 * STEP text export payload returned by export helpers.
 */
export interface OGStepExportResult {
  text: string;
  reportJson: string;
}

/**
 * IFC text export payload returned by export helpers.
 */
export interface OGIfcExportResult {
  text: string;
  reportJson: string;
}

/**
 * Shared runtime entrypoint used to initialize the OpenGeometry wasm module.
 */
export class OpenGeometry {
  static version = OPEN_GEOMETRY_THREE_VERSION;
  static instance: OpenGeometry | null = null;

  private _enableDebug: boolean = false;

  get enableDebug() {
    return this._enableDebug;
  }

  /**
   * Enables or disables debug mode for OpenGeometry.
   * When enabled, it logs debug information to the console.
   * Addtionally,
   * 1. The geometry will be rendered with a semi-transparent material.
   * 2. The faces will be rendered with a random color.
   * 3. The normals will be rendered for better visualization.
   * @param value - A boolean indicating whether to enable or disable debug mode.
   */
  set enableDebug(value: boolean) {
    this._enableDebug = value;
    if (this._enableDebug) {
      console.log("OpenGeometry Debug Mode Enabled");
    }
  }

  constructor() {}

  /**
   * Asynchronously initializes the OpenGeometry wasm runtime and returns a
   * cached singleton instance on subsequent calls.
   *
   * Call this before constructing `Vector3` or any kernel-backed wrapper.
   *
   * @param options - Configuration object for initializing OpenGeometry.
   * @returns A promise that resolves to a fully initialized OpenGeometry instance.
   *
   * @example
   * ```ts
   * await OpenGeometry.create({
   *   wasmURL: "/opengeometry_bg.wasm",
   * });
   * ```
   */
  static async create(options: OpenGeometryOptions) {
    if (!OpenGeometry.instance) {
      const og = new OpenGeometry();
      await og.setup(options.wasmURL);
      OpenGeometry.instance = og;
    }
    return OpenGeometry.instance;
  }

  private async setup(wasmURL?: string) {
    if (wasmURL) {
      await init({ module_or_path: wasmURL });
      return;
    }

    await init();
  }
}

/**
 * Scene manager that stores serialized BRep snapshots in wasm for projection,
 * export, and other scene-level workflows.
 */
export { OGSceneManager };

/**
 * Shared wasm-backed vector type used throughout the public API.
 *
 * Initialize `OpenGeometry.create(...)` before constructing `Vector3`.
 */
export { Vector3 };

export { SpotLabel };

/**
 * Primitive wrappers (line/polyline/arc/rectangle/curve).
 */
export * from "./src/primitives/index";
/**
 * Shape wrappers (polygon/cuboid/cylinder/wedge/opening/sweep/sphere).
 */
export * from "./src/shapes/index";
/**
 * Reusable example builders for quickly wiring demo scenes.
 */
export * from "./src/examples/index";
/**
 * First-class freeform geometry wrapper around the kernel OGFreeformGeometry API.
 */
export * from "./src/freeform/index";
/**
 * Editing helpers that operate on parametric entities and freeform geometry.
 */
export * from "./src/editor/index";
/**
 * Kernel-backed modeling operations.
 */
export * from "./src/operations/index";
