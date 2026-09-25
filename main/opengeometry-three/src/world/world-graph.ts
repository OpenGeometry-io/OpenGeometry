import * as THREE from "three";
import { OGWorldGraph } from "../../../opengeometry/pkg/opengeometry";
import { uniformScale } from "../rendering/analytic-lod";
import type { AnalyticFrame, AnalyticProjectionCamera, AnalyticProjectionHlr, AnalyticSolid } from "../shapes/analytic-solid";

export interface WorldTransform {
  frame: AnalyticFrame;
  scale: number;
}

export interface WorldNodeOptions {
  id: string;
  parent?: string;
  definition?: string;
  local?: THREE.Matrix4;
  kind?: string;
}

export interface WorldProjectedLine {
  start: { x: number; y: number };
  end: { x: number; y: number };
  stroke_width: number | null;
  stroke_color: [number, number, number] | null;
  class?: string;
}

export interface WorldProjectedLines {
  name: string | null;
  lines: WorldProjectedLine[];
}

export interface WorldProjectedSegment {
  geometry: unknown;
  class: string;
  layer?: string | null;
  source_entity_id?: string | null;
}

export interface WorldProjectedScene {
  name: string | null;
  segments: WorldProjectedSegment[];
}

export interface WorldProjectionView {
  id: string;
  camera: AnalyticProjectionCamera;
  hlr?: AnalyticProjectionHlr;
}

export interface WorldExportedMesh {
  bytes: Uint8Array;
  report: unknown;
}

export interface WorldExportedFile {
  text: string;
  report: unknown;
}

export type WorldClashKind = "hard" | "touch" | "unresolved";

export interface WorldClash {
  a: string;
  b: string;
  kind: WorldClashKind;
  parts: [string, string];
  detail?: string;
}

export interface WorldClashOptions {
  depth?: number;
}

export interface WorldProximity {
  a: string;
  b: string;
  distance: number;
  errorBound: number;
  parts: [string, string];
  points: [[number, number, number], [number, number, number]];
}

export type WorldGraphErrorCode =
  | "duplicate_node"
  | "unknown_node"
  | "duplicate_definition"
  | "unknown_definition"
  | "definition_in_use"
  | "cycle"
  | "invalid_input"
  | "export"
  | "geometry"
  | "unknown";

export class WorldGraphError extends Error {
  readonly code: WorldGraphErrorCode;
  readonly detail: unknown;

  constructor(code: WorldGraphErrorCode, message: string, detail?: unknown) {
    super(message);
    this.name = "WorldGraphError";
    this.code = code;
    this.detail = detail;
  }
}

const KNOWN_CODES = new Set<WorldGraphErrorCode>([
  "duplicate_node", "unknown_node", "duplicate_definition", "unknown_definition",
  "definition_in_use", "cycle", "invalid_input", "export", "geometry",
]);

function toWorldGraphError(raw: unknown): WorldGraphError {
  const text = typeof raw === "string" ? raw : raw instanceof Error ? raw.message : String(raw);
  try {
    const payload = JSON.parse(text) as unknown;
    if (typeof payload === "object" && payload !== null && !Array.isArray(payload)) {
      const variant = Object.keys(payload)[0];
      if (variant) {
        const detail = (payload as Record<string, unknown>)[variant];
        const snake = variant.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase() as WorldGraphErrorCode;
        const code = KNOWN_CODES.has(snake) ? snake : "unknown";
        const message = typeof detail === "string" ? `${snake.replace(/_/g, " ")}: ${detail}` : snake.replace(/_/g, " ");
        return new WorldGraphError(code, message, detail);
      }
    }
  } catch {
    return new WorldGraphError("unknown", text, raw);
  }
  return new WorldGraphError("unknown", text, raw);
}

function kernelCall<T>(call: () => T): T {
  try {
    return call();
  } catch (error) {
    throw toWorldGraphError(error);
  }
}

export function matrixToWorldTransform(matrix: THREE.Matrix4): WorldTransform {
  const scale = uniformScale(matrix);
  const elements = matrix.elements;
  const axis = (column: number): [number, number, number] => [
    elements[column * 4] / scale, elements[column * 4 + 1] / scale, elements[column * 4 + 2] / scale,
  ];
  return {
    frame: { origin: [elements[12], elements[13], elements[14]], x: axis(0), y: axis(1), z: axis(2) },
    scale,
  };
}

export class WorldGraph {
  private readonly kernel = new OGWorldGraph();
  private readonly bindings = new Map<string, THREE.Object3D>();
  private bindingOrder: [string, THREE.Object3D][] | undefined;
  private disposed = false;

  get nodeCount(): number {
    this.checkActive();
    return this.kernel.node_count();
  }

  get definitionCount(): number {
    this.checkActive();
    return this.kernel.definition_count();
  }

  define(id: string, solid: AnalyticSolid): void {
    this.checkActive();
    const brep = solid.getBrepSerialized();
    kernelCall(() => this.kernel.define(id, brep));
  }

  defineLegacy(id: string, brepSerialized: string): void {
    this.checkActive();
    kernelCall(() => this.kernel.define_legacy(id, brepSerialized));
  }

  hasDefinition(id: string): boolean {
    this.checkActive();
    return this.kernel.has_definition(id);
  }

  undefine(id: string): void {
    this.checkActive();
    kernelCall(() => this.kernel.undefine(id));
  }

  addNode(options: WorldNodeOptions): void {
    this.checkActive();
    const spec = JSON.stringify({
      id: options.id,
      parent: options.parent,
      definition: options.definition,
      local: matrixToWorldTransform(options.local ?? new THREE.Matrix4()),
      kind: options.kind,
    });
    kernelCall(() => this.kernel.add_node(spec));
  }

  hasNode(id: string): boolean {
    this.checkActive();
    return this.kernel.has_node(id);
  }

  setLocal(id: string, local: THREE.Matrix4): void {
    this.checkActive();
    const transform = JSON.stringify(matrixToWorldTransform(local));
    kernelCall(() => this.kernel.set_local(id, transform));
    this.sync();
  }

  reparent(id: string, parent?: string): void {
    this.checkActive();
    kernelCall(() => this.kernel.reparent(id, parent));
    this.sync();
  }

  removeNode(id: string): void {
    this.checkActive();
    kernelCall(() => this.kernel.remove_node(id));
    for (const nodeId of [...this.bindings.keys()]) {
      if (!this.kernel.has_node(nodeId)) this.bindings.delete(nodeId);
    }
    this.bindingOrder = undefined;
  }

  getWorldMatrix(id: string): THREE.Matrix4 {
    this.checkActive();
    return new THREE.Matrix4().fromArray(kernelCall(() => this.kernel.get_world(id)));
  }

  getWorldBounds(id: string): THREE.Box3 | null {
    this.checkActive();
    const bounds = kernelCall(() => this.kernel.get_world_bounds(id));
    if (bounds.length !== 6) return null;
    return new THREE.Box3(
      new THREE.Vector3(bounds[0], bounds[1], bounds[2]),
      new THREE.Vector3(bounds[3], bounds[4], bounds[5]),
    );
  }

  clash(itemsA: string[], itemsB: string[], options: WorldClashOptions = {}): WorldClash[] {
    this.checkActive();
    const depth = options.depth ?? 0.001;
    return JSON.parse(kernelCall(() => this.kernel.clash(JSON.stringify(itemsA), JSON.stringify(itemsB), depth))) as WorldClash[];
  }

  clearance(itemsA: string[], itemsB: string[], clearance: number): WorldProximity[] {
    this.checkActive();
    return JSON.parse(kernelCall(() => this.kernel.clearance(JSON.stringify(itemsA), JSON.stringify(itemsB), clearance))) as WorldProximity[];
  }

  distance(itemA: string, itemB: string, tolerance = 0.001): WorldProximity | null {
    this.checkActive();
    return JSON.parse(kernelCall(() => this.kernel.distance(itemA, itemB, tolerance))) as WorldProximity | null;
  }

  projectLines(items: string[], camera: AnalyticProjectionCamera, hlr?: AnalyticProjectionHlr, deflection = 0.01): WorldProjectedLines {
    this.checkActive();
    const hlrJson = hlr === undefined ? undefined : JSON.stringify(hlr);
    return JSON.parse(kernelCall(() => this.kernel.project_lines(JSON.stringify(items), JSON.stringify(camera), hlrJson, deflection))) as WorldProjectedLines;
  }

  projectViews(items: string[], views: WorldProjectionView[], deflection = 0.01): Record<string, WorldProjectedScene> {
    this.checkActive();
    return JSON.parse(kernelCall(() => this.kernel.project_views(JSON.stringify(items), JSON.stringify(views), deflection))) as Record<string, WorldProjectedScene>;
  }

  exportStl(items: string[], config?: Record<string, unknown>): WorldExportedMesh {
    this.checkActive();
    const result = kernelCall(() => this.kernel.export_stl(JSON.stringify(items), config && JSON.stringify(config)));
    try {
      return { bytes: result.bytes, report: JSON.parse(result.reportJson) };
    } finally {
      result.free();
    }
  }

  exportStep(items: string[], config?: Record<string, unknown>): WorldExportedFile {
    this.checkActive();
    const result = kernelCall(() => this.kernel.export_step(JSON.stringify(items), config && JSON.stringify(config)));
    try {
      return { text: result.text, report: JSON.parse(result.reportJson) };
    } finally {
      result.free();
    }
  }

  exportIfc(items: string[], config?: Record<string, unknown>): WorldExportedFile {
    this.checkActive();
    const result = kernelCall(() => this.kernel.export_ifc(JSON.stringify(items), config && JSON.stringify(config)));
    try {
      return { text: result.text, report: JSON.parse(result.reportJson) };
    } finally {
      result.free();
    }
  }

  bind(object: THREE.Object3D, id: string): void {
    this.checkActive();
    if (!this.kernel.has_node(id)) throw new WorldGraphError("unknown_node", `unknown node: ${id}`, id);
    object.matrixAutoUpdate = false;
    this.bindings.set(id, object);
    this.bindingOrder = undefined;
    this.syncObject(id, object);
  }

  unbind(id: string): void {
    this.checkActive();
    this.bindings.delete(id);
    this.bindingOrder = undefined;
  }

  sync(): void {
    this.checkActive();
    this.bindingOrder ??= [...this.bindings].sort(([, a], [, b]) => depth(a) - depth(b));
    for (const [id, object] of this.bindingOrder) this.syncObject(id, object);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.bindings.clear();
    this.bindingOrder = undefined;
    this.kernel.free();
  }

  private syncObject(id: string, object: THREE.Object3D): void {
    const world = this.getWorldMatrix(id);
    if (object.parent) {
      object.parent.updateWorldMatrix(true, false);
      world.premultiply(new THREE.Matrix4().copy(object.parent.matrixWorld).invert());
    }
    object.matrix.copy(world);
    object.matrix.decompose(object.position, object.quaternion, object.scale);
    object.updateMatrixWorld(true);
  }

  private checkActive(): void {
    if (this.disposed) throw new WorldGraphError("invalid_input", "WorldGraph has been disposed");
  }
}

function depth(object: THREE.Object3D): number {
  let count = 0;
  for (let parent = object.parent; parent; parent = parent.parent) count += 1;
  return count;
}
