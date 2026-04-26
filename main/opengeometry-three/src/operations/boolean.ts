import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { toCreasedNormals } from "three/examples/jsm/utils/BufferGeometryUtils.js";

import {
  createFreeformGeometry,
  type CreateFreeformGeometryOptions,
  type FreeformGeometry,
} from "../freeform";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "../shapes/outline-utils";

/**
 * Accepted operand formats for boolean helpers.
 */
export type BooleanOperand =
  | string
  | Record<string, unknown>
  | {
      getBrep?: () => unknown;
      getBrepData?: () => unknown;
      getBrepSerialized?: () => string;
    };

/**
 * Kernel-side boolean tuning options.
 */
export interface BooleanKernelOptions {
  tolerance?: number;
  mergeCoplanarFaces?: boolean;
}

/**
 * Rendering options applied to the boolean result mesh.
 *
 * `color`, `opacity`, `transparent`, and `side` override the corresponding
 * fields on the host (LHS) operand's cloned material. Omit them to inherit
 * the host's appearance unchanged.
 */
export interface BooleanRenderOptions {
  color?: number;
  opacity?: number;
  transparent?: boolean;
  side?: THREE.Side;
  outline?: boolean;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

/**
 * Combined kernel and rendering options accepted by boolean helpers.
 */
export interface BooleanExecutionOptions extends BooleanRenderOptions {
  kernel?: BooleanKernelOptions;
}

/**
 * Structured report returned by the kernel boolean pipeline.
 */
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

type KernelBooleanManyFunction = (
  lhsBrepSerialized: string,
  rhsBrepsSerialized: string,
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
    options?: BooleanRenderOptions,
    sourceMaterial: THREE.Material | null = null
  ) {
    super();
    this.ogid = getUUID();
    this._fatOutlines = options?.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(options?.outlineWidth);
    this.applyKernelResult(kernelResult, options, sourceMaterial);

    if (options?.outline ?? false) {
      this.outline = true;
    }
  }

  /**
   * Rebuilds the mesh, cached BRep payload, and outline geometry from the kernel output.
   */
  applyKernelResult(
    kernelResult: KernelBooleanResult,
    options?: BooleanRenderOptions,
    sourceMaterial: THREE.Material | null = null
  ) {
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

    const material = buildResultMaterial(sourceMaterial, options);

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

  canConvertToFreeform() {
    return this.brepSerialized.length > 0;
  }

  /**
   * Converts the boolean output into a first-class freeform geometry object so
   * it can participate in direct face/edge/vertex editing.
   */
  toFreeform(options?: CreateFreeformGeometryOptions): FreeformGeometry {
    if (!this.canConvertToFreeform()) {
      throw new Error("This boolean result cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.getBrepSerialized(), {
      id: options?.id ?? this.ogid,
      placement: options?.placement,
    });
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
 *
 * Accepts wrapper objects directly, including `Solid`, `Cuboid`, `Opening`,
 * and other shapes that expose BRep accessors.
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
 *
 * Use this when you want a standalone helper rather than a shape-level
 * convenience method like `opening.subtractFrom(host)`.
 */
export function booleanSubtraction(
  lhs: BooleanOperand,
  rhs: BooleanOperand,
  options?: BooleanExecutionOptions
) {
  return executeBoolean("booleanSubtraction", lhs, rhs, options);
}

/**
 * Computes a kernel-backed repeated subtraction using a left-to-right cutter array.
 *
 * This is used by the shape-level `.subtract([...])` helpers so the heavy
 * boolean orchestration stays in the Rust/WASM layer instead of JavaScript.
 */
export function executeBooleanSubtractionMany(
  lhs: BooleanOperand,
  cutters: BooleanOperand[],
  options?: BooleanExecutionOptions
) {
  const booleanExport = (OGKernel as Record<string, unknown>).booleanSubtractionMany;
  if (typeof booleanExport !== "function") {
    throw new Error(
      "booleanSubtractionMany is not available in the loaded wasm package. Rebuild opengeometry wasm bindings."
    );
  }

  const kernelFunction = booleanExport as KernelBooleanManyFunction;
  const kernelOptions = serializeKernelOptions(options?.kernel);
  const kernelResult = kernelFunction(
    resolveOperandSerialized(lhs),
    JSON.stringify(cutters.map(resolveOperandPayload)),
    kernelOptions
  );

  return createBooleanResult(kernelResult, options, resolveOperandMaterial(lhs));
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

  return createBooleanResult(kernelResult, options, resolveOperandMaterial(lhs));
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
 * Parses a boolean operand into a raw BRep payload so array-backed boolean
 * calls can serialize a JSON array of BReps instead of a JSON array of strings.
 */
function resolveOperandPayload(operand: BooleanOperand) {
  const serialized = resolveOperandSerialized(operand);
  try {
    return JSON.parse(serialized) as Record<string, unknown>;
  } catch {
    throw new Error(
      "Boolean operands must resolve to valid serialized BRep JSON before array subtraction."
    );
  }
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

function createBooleanResult(
  kernelResult: KernelBooleanResult,
  options?: BooleanExecutionOptions,
  sourceMaterial: THREE.Material | null = null
) {
  const result = new BooleanResult(kernelResult, options, sourceMaterial);
  if (options?.outline ?? true) {
    result.outline = true;
  }
  return result;
}

/**
 * Returns a clonable `THREE.Material` from the operand when it is a Three mesh
 * subclass (Solid, Polygon, Cuboid, Opening, BooleanResult, etc.). Returns
 * `null` for serialized JSON, parsed BRep objects, or anything else without a
 * reachable material so the boolean result can fall back to the legacy default.
 */
function resolveOperandMaterial(operand: BooleanOperand): THREE.Material | null {
  if (typeof operand !== "object" || operand === null) {
    return null;
  }

  if (!(operand instanceof THREE.Mesh)) {
    return null;
  }

  const material = (operand as THREE.Mesh).material;
  if (Array.isArray(material)) {
    return material[0] ?? null;
  }

  return material instanceof THREE.Material ? material : null;
}

/**
 * Builds the boolean result mesh's material by cloning the host (LHS) material
 * when available so the result inherits the host's color, transparency, and
 * side mode. Caller-provided overrides on `BooleanRenderOptions` win, and the
 * polygon-offset settings that prevent z-fighting with the result outline are
 * always reapplied after cloning.
 */
function buildResultMaterial(
  sourceMaterial: THREE.Material | null,
  options?: BooleanRenderOptions
): THREE.Material {
  const material = sourceMaterial
    ? sourceMaterial.clone()
    : new THREE.MeshStandardMaterial({
        color: 0x2563eb,
        transparent: true,
        opacity: 0.82,
        side: THREE.FrontSide,
      });

  if (options?.color !== undefined && "color" in material) {
    (material as THREE.MeshStandardMaterial | THREE.MeshBasicMaterial).color.set(
      options.color
    );
  }
  if (options?.opacity !== undefined) {
    material.opacity = options.opacity;
    material.transparent = options.opacity < 1 || material.transparent;
  }
  if (options?.transparent !== undefined) {
    material.transparent = options.transparent;
  }
  if (options?.side !== undefined) {
    material.side = options.side;
  }

  material.polygonOffset = true;
  material.polygonOffsetFactor = 1;
  material.polygonOffsetUnits = 1;
  material.needsUpdate = true;
  return material;
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
