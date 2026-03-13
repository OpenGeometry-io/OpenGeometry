import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { toCreasedNormals } from "three/examples/jsm/utils/BufferGeometryUtils.js";

import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "../shapes/outline-utils";

export type BooleanOperand =
  | string
  | Record<string, unknown>
  | {
      getBrep?: () => unknown;
      getBrepData?: () => unknown;
      getBrepSerialized?: () => string;
    };

export interface BooleanKernelOptions {
  tolerance?: number;
  mergeCoplanarFaces?: boolean;
}

export interface BooleanRenderOptions {
  color?: number;
  outline?: boolean;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

export interface BooleanExecutionOptions extends BooleanRenderOptions {
  kernel?: BooleanKernelOptions;
}

export interface BooleanReport {
  operation: string;
  operand_kind: string;
  input_face_count: number;
  input_triangle_count: number;
  output_face_count: number;
  output_shell_count: number;
  empty: boolean;
}

interface KernelBooleanResult {
  brepSerialized: string;
  geometrySerialized: string;
  outlineGeometrySerialized: string;
  reportJson: string;
}

type KernelBooleanFunction = (
  lhsBrepSerialized: string,
  rhsBrepSerialized: string,
  optionsJson?: string
) => KernelBooleanResult;

const BOOLEAN_CREASE_ANGLE = Math.PI / 5;
const BOOLEAN_OUTLINE_THRESHOLD_DEGREES = 26;

/**
 * Renderable result mesh returned by the kernel boolean helpers.
 */
export class BooleanResult extends THREE.Mesh {
  ogid: string;
  report: BooleanReport | null = null;

  private brepSerialized = "";
  private brepData: Record<string, unknown> | null = null;
  private outlinePositions: number[] = [];
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;
  #outlineMesh: ShapeOutlineMesh | null = null;

  constructor(
    kernelResult: KernelBooleanResult,
    options?: BooleanRenderOptions
  ) {
    super();
    this.ogid = getUUID();
    this._fatOutlines = options?.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(options?.outlineWidth);
    this.applyKernelResult(kernelResult, options?.color ?? 0x2563eb);

    if (options?.outline ?? false) {
      this.outline = true;
    }
  }

  /**
   * Rebuilds the mesh, cached BRep payload, and outline geometry from the kernel output.
   */
  applyKernelResult(kernelResult: KernelBooleanResult, color: number = 0x2563eb) {
    this.disposeGeometry();

    this.brepSerialized = kernelResult.brepSerialized;
    this.brepData = JSON.parse(kernelResult.brepSerialized) as Record<string, unknown>;
    this.report = JSON.parse(kernelResult.reportJson) as BooleanReport;

    const positions = JSON.parse(kernelResult.geometrySerialized) as number[];
    const baseGeometry = new THREE.BufferGeometry();
    baseGeometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(positions, 3)
    );
    const geometry = toCreasedNormals(baseGeometry, BOOLEAN_CREASE_ANGLE);
    geometry.computeBoundingBox();
    this.outlinePositions = getRenderableOutlinePositions(
      geometry,
      kernelResult.outlineGeometrySerialized
    );

    const material = new THREE.MeshStandardMaterial({
      color,
      transparent: true,
      opacity: 0.82,
      side: THREE.FrontSide,
      polygonOffset: true,
      polygonOffsetFactor: 1,
      polygonOffsetUnits: 1,
    });

    this.geometry = geometry;
    this.material = material;

    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  /**
   * Returns the parsed BRep object for reuse with scene export or downstream operations.
   */
  getBrepData() {
    return this.brepData;
  }

  /**
   * Returns the serialized BRep JSON emitted by the kernel boolean operation.
   */
  getBrepSerialized() {
    return this.brepSerialized;
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (!enable || this.outlinePositions.length === 0) {
      return;
    }

    this.#outlineMesh = createShapeOutlineMesh({
      positions: this.outlinePositions,
      color: 0x000000,
      fatOutlines: this._fatOutlines,
      outlineWidth: this._outlineWidth,
    });
    this.add(this.#outlineMesh);
  }

  get outline() {
    return this._outlineEnabled;
  }

  set fatOutlines(value: boolean) {
    this._fatOutlines = value;
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  get fatOutlines() {
    return this._fatOutlines;
  }

  set outlineWidth(value: number) {
    this._outlineWidth = sanitizeOutlineWidth(value);
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  get outlineWidth() {
    return this._outlineWidth;
  }

  disposeGeometry() {
    this.clearOutlineMesh();
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach((material) => material.dispose());
      return;
    }
    this.material.dispose();
  }

  private clearOutlineMesh() {
    if (!this.#outlineMesh) {
      return;
    }
    this.remove(this.#outlineMesh);
    disposeShapeOutlineMesh(this.#outlineMesh);
    this.#outlineMesh = null;
  }
}

/**
 * Computes a kernel-backed boolean union and returns a renderable result mesh.
 */
export function booleanUnion(
  lhs: BooleanOperand,
  rhs: BooleanOperand,
  options?: BooleanExecutionOptions
) {
  return executeBoolean("booleanUnion", lhs, rhs, options);
}

/**
 * Computes a kernel-backed boolean intersection and returns a renderable result mesh.
 */
export function booleanIntersection(
  lhs: BooleanOperand,
  rhs: BooleanOperand,
  options?: BooleanExecutionOptions
) {
  return executeBoolean("booleanIntersection", lhs, rhs, options);
}

/**
 * Computes a kernel-backed boolean subtraction and returns a renderable result mesh.
 */
export function booleanSubtraction(
  lhs: BooleanOperand,
  rhs: BooleanOperand,
  options?: BooleanExecutionOptions
) {
  return executeBoolean("booleanSubtraction", lhs, rhs, options);
}

/**
 * Normalizes mixed wrapper/raw operands, invokes the wasm boolean export, and
 * returns the renderable result wrapper.
 */
function executeBoolean(
  exportName: "booleanUnion" | "booleanIntersection" | "booleanSubtraction",
  lhs: BooleanOperand,
  rhs: BooleanOperand,
  options?: BooleanExecutionOptions
) {
  const booleanExport = (OGKernel as Record<string, unknown>)[exportName];
  if (typeof booleanExport !== "function") {
    throw new Error(
      `${exportName} is not available in the loaded wasm package. Rebuild opengeometry wasm bindings.`
    );
  }

  const kernelFunction = booleanExport as KernelBooleanFunction;
  const kernelOptions = serializeKernelOptions(options?.kernel);
  const kernelResult = kernelFunction(
    resolveOperandSerialized(lhs),
    resolveOperandSerialized(rhs),
    kernelOptions
  );

  const result = new BooleanResult(kernelResult, options);
  if (options?.outline ?? true) {
    result.outline = true;
  }
  return result;
}

/**
 * Accepts wrapper objects, raw BRep data, or serialized JSON and normalizes
 * everything into the kernel's serialized input format.
 */
function resolveOperandSerialized(operand: BooleanOperand): string {
  if (typeof operand === "string") {
    return operand;
  }

  if (typeof operand.getBrepSerialized === "function") {
    return serializeBrepLike(operand.getBrepSerialized());
  }

  if (typeof operand.getBrepData === "function") {
    return serializeBrepLike(operand.getBrepData());
  }

  if (typeof operand.getBrep === "function") {
    return serializeBrepLike(operand.getBrep());
  }

  return serializeBrepLike(operand);
}

/**
 * Accepts either a parsed BRep object or an already serialized JSON payload
 * and always returns the canonical serialized form expected by the wasm API.
 */
function serializeBrepLike(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }

  return JSON.stringify(value);
}

/**
 * Translates the camelCase Three-side options into the snake_case wasm payload.
 */
function serializeKernelOptions(options?: BooleanKernelOptions) {
  if (!options) {
    return undefined;
  }

  return JSON.stringify({
    tolerance: options.tolerance,
    merge_coplanar_faces: options.mergeCoplanarFaces,
  });
}

/**
 * Builds a display-quality outline buffer from the rendered mesh and falls back
 * to the kernel-provided edges when the client-side extraction is empty.
 */
function getRenderableOutlinePositions(
  geometry: THREE.BufferGeometry,
  fallbackSerialized: string
) {
  const edgesGeometry = new THREE.EdgesGeometry(
    geometry,
    BOOLEAN_OUTLINE_THRESHOLD_DEGREES
  );
  const positionsAttribute = edgesGeometry.getAttribute("position");
  const positions =
    positionsAttribute && positionsAttribute.count > 0
      ? Array.from(positionsAttribute.array as ArrayLike<number>)
      : [];
  edgesGeometry.dispose();

  if (positions.length > 0) {
    return positions;
  }

  return JSON.parse(fallbackSerialized) as number[];
}
