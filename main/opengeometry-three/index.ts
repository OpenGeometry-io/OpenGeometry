/**
 * OpenGeometry Three.js adapter entrypoint.
 * @module @opengeometry/kernel-three
 */
import init, {
  OGSceneManager,
  Vector3,
} from "../opengeometry/pkg/opengeometry";
// Vector3 is also available in opengeometry package
// import { Vector3 } from "@opengeometry/openmaths";
import { SpotLabel } from "./src/markup/spotMarker";
import {
  AXISMODE,
  OPEN_GEOMETRY_THREE_VERSION,
  OpenGeometryOptions,
} from "./src/base-types";

export type OUTLINE_TYPE = "front" | "side" | "top";

export interface OGStlExportResult {
  bytes: Uint8Array;
  reportJson: string;
}

export interface OGStepExportResult {
  text: string;
  reportJson: string;
}

export interface OGIfcExportResult {
  text: string;
  reportJson: string;
}

export class OpenGeometry {
  static version = OPEN_GEOMETRY_THREE_VERSION;
  static instance: OpenGeometry | null = null;
  private static _axisMode: AXISMODE | null = null;

  private _enableDebug: boolean = false;

  get enableDebug() {
    return this._enableDebug;
  }

  static get axisMode() {
    return OpenGeometry._axisMode;
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
   * Asynchronously creates and initializes an instance of OpenGeometry.
   *
   * This factory method sets up the OpenGeometry engine by linking it with the
   * rendering context and the WebAssembly module. It ensures all required
   * options are provided and prepares the instance for 3D geometry operations.
   *
   * @param options - Configuration object for initializing OpenGeometry.
   * @returns A promise that resolves to a fully initialized OpenGeometry instance.
   * @throws If any of the required options (`container`, `scene`, or `camera`) are missing.
   *
   * @example
   * ```ts
   * const openGeometry = await OpenGeometry.create({
   *   wasmURL: '/assets/opengeometry.wasm',
   *   axisMode: AXISMODE.ZFRONT
   * });
   * ```
   */
  static async create(options: OpenGeometryOptions = {}) {
    const resolvedAxisMode = OpenGeometry.resolveAxisMode(options.axisMode);

    if (
      OpenGeometry._axisMode !== null &&
      resolvedAxisMode !== OpenGeometry._axisMode
    ) {
      throw new Error(
        `OpenGeometry is already initialized with axisMode '${OpenGeometry._axisMode}'. ` +
          `Received '${resolvedAxisMode}'. axisMode is global and cannot be changed after initialization.`
      );
    }

    OpenGeometry._axisMode = resolvedAxisMode;

    if (!OpenGeometry.instance) {
      const og = new OpenGeometry();
      await og.setup(options.wasmURL);
      OpenGeometry.instance = og;
    }
    return OpenGeometry.instance;
  }

  private static resolveAxisMode(inputAxisMode?: AXISMODE): AXISMODE {
    if (inputAxisMode === undefined && OpenGeometry._axisMode !== null) {
      return OpenGeometry._axisMode;
    }

    const axisMode = inputAxisMode === undefined ? AXISMODE.ZFRONT : inputAxisMode;
    const objectWithValues = Object as unknown as {
      values<T extends object>(obj: T): Array<T[keyof T]>;
    };
    const validAxisModes = objectWithValues.values(AXISMODE);
    if (validAxisModes.indexOf(axisMode) === -1) {
      throw new Error(
        `Invalid axisMode '${String(axisMode)}'. Allowed values: ${validAxisModes.join(", ")}.`
      );
    }

    return axisMode;
  }

  private async setup(wasmURL?: string) {
    await init(wasmURL);
  }
}

export {
  AXISMODE,
  OGSceneManager,
  Vector3,
  SpotLabel,
}
export type {
  OpenGeometryOptions,
}

/**
 * Primitive wrappers (line/polyline/arc/rectangle/curve).
 */
export * from './src/primitives/';
/**
 * Shape wrappers (polygon/cuboid/cylinder/wedge/opening/sweep/sphere).
 */
export * from './src/shapes/';
/**
 * Reusable example builders for quickly wiring demo scenes.
 */
export * from './src/examples/';
