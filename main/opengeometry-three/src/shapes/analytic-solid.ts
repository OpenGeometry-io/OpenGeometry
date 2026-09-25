import * as THREE from "three";
import { OGAnalyticBrep, tessellate_brep } from "../../../opengeometry/pkg/opengeometry";
import { getUUID } from "../utils/randomizer";
import { GEOMETRY_DEFLECTION_SHARE, type AnalyticTessellationData } from "../rendering/analytic-tessellation";
import { matrixToWorldTransform } from "../world/world-graph";
import { analyticIfcText, AnalyticExchangeBodyV2 } from "../export/analytic-ifc";
import { parseAnalyticGeometryError } from "../operations/analytic-errors";

export interface AnalyticFrame {
  origin: [number, number, number];
  x: [number, number, number];
  y: [number, number, number];
  z: [number, number, number];
}

export interface AnalyticAccuracy {
  geometric: number;
  intersection: number;
  tessellation: number;
  exchange: number;
}

export interface AnalyticAnnularSectorOpening {
  id: string;
  angle: number;
  width: number;
  bottom: number;
  height: number;
}

export type AnalyticProfileEdge =
  | { kind: "line"; from: [number, number]; to: [number, number] }
  | { kind: "arc"; center: [number, number]; radius: number; startAngle: number; sweepAngle: number };

export type AnalyticPolygonLoftAlignment = "auto" | {
  upperStartIndex: number;
  reverseUpper?: boolean;
};

export interface AnalyticProjectionCamera {
  position: { x: number; y: number; z: number };
  target: { x: number; y: number; z: number };
  up: { x: number; y: number; z: number };
  near: number;
  projection_mode: "Orthographic" | "Perspective";
}

export interface AnalyticProjectionHlr {
  hide_hidden_edges: boolean;
}

interface AnalyticOptions {
  ogid?: string;
  frame?: AnalyticFrame;
  accuracy?: AnalyticAccuracy;
  deflection?: number;
  color?: THREE.ColorRepresentation;
}

export type AnalyticPrimitiveOptions = AnalyticOptions & (
  | { kind: "cuboid"; width: number; depth: number; height: number }
  | { kind: "linearExtrusion"; outer: [number, number][]; holes: [number, number][][]; height: number }
  | { kind: "arcEdgedExtrusion"; outer: AnalyticProfileEdge[]; holes?: AnalyticProfileEdge[][]; height: number }
  | { kind: "polygonLoft"; lower: [number, number, number][]; upper: [number, number, number][]; alignment?: AnalyticPolygonLoftAlignment }
  | { kind: "boxWithArchedOpening"; width: number; depth: number; height: number; opening: { id: string; station: number; width: number; bottom: number; height: number } }
  | { kind: "planarPolyhedron"; vertices: [number, number, number][]; faces: number[][] }
  | { kind: "cylinder" | "cone"; radius: number; height: number }
  | { kind: "cylinderSector"; radius: number; height: number; startAngle: number; sweepAngle: number }
  | { kind: "sphere"; radius: number }
  | { kind: "frustum"; lowerRadius: number; upperRadius: number; height: number }
  | { kind: "torus"; majorRadius: number; minorRadius: number }
  | { kind: "annularSectorExtrusion"; radius: number; thickness: number; height: number; startAngle: number; sweepAngle: number }
  | { kind: "annularSectorExtrusionWithOpenings"; radius: number; thickness: number; height: number; startAngle: number; sweepAngle: number; openings: AnalyticAnnularSectorOpening[] }
  | { kind: "annularSectorExtrusionWithArchedOpening"; radius: number; thickness: number; height: number; startAngle: number; sweepAngle: number; opening: AnalyticAnnularSectorOpening }
  | { kind: "coaxialCircleLoft"; lowerRadius: number; upperRadius: number; height: number }
  | { kind: "revolvedRectangle"; innerRadius: number; outerRadius: number; height: number }
  | { kind: "chamferedCuboid"; width: number; depth: number; height: number; chamfer: number }
  | { kind: "filletedCylinder"; radius: number; height: number; filletRadius: number }
);

export type AnalyticBooleanOp = "union" | "intersection" | "subtraction";
export type AnalyticPointClassification = "inside" | "outside" | "boundary" | "unknown";

export interface AnalyticBooleanReport {
  operation: AnalyticBooleanOp;
  quality: { kind: "Analytic" };
  contacts: [number, number, number][];
  coincident: boolean;
  face_mappings: {
    source: { entity: string; body: string; key: string; face: number };
    result_faces: number[];
  }[];
}

export interface AnalyticBrepOptions extends Omit<AnalyticOptions, "ogid" | "frame" | "accuracy"> {
  kind: "brep";
  serialized: string;
}

export interface AnalyticTessellationStats {
  requestedDeflection: number;
  achievedDeflection: number;
  triangleCount: number;
  faceCount: number;
  revision: bigint;
}

export interface AnalyticTessellationOptions {
  maxTriangles?: number;
}

export interface AnalyticStepExport {
  text: string;
  report: {
    schema_version: 2;
    revision: string;
    length_unit: "metre" | "millimetre";
    quality: { kind: string };
    faces: number;
    edges: number;
    solids: number;
    cavity_shells: number;
    collapsed_chart_uses: number;
    geometric_tolerance: number;
    exchange_error_bound: number;
    validation_level: string;
  };
}

export interface AnalyticStlExport {
  bytes: Uint8Array;
  report: {
    schema_version: 2;
    revision: string;
    triangle_count: number;
    requested_deflection: number;
    achieved_deflection: number;
    float32_encoding_error: number;
    quality: "tessellated_from_analytic_brep";
  };
}

export class AnalyticSolid extends THREE.Group {
  readonly ogid: string;
  readonly surface: THREE.Mesh<THREE.BufferGeometry, THREE.MeshStandardMaterial>;
  readonly featureOutline: THREE.LineSegments<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  private kernel: OGAnalyticBrep;
  private triangleFaces = new Uint32Array(0);
  private outlineEdges = new Uint32Array(0);
  private disposed = false;
  private lastDeflection = 0.001;
  private booleanReport?: AnalyticBooleanReport;

  get report(): AnalyticBooleanReport | undefined { return this.booleanReport; }
  get isDisposed(): boolean { return this.disposed; }
  get modelRevision(): bigint { this.checkActive(); return this.kernel.get_revision(); }
  get deflection(): number { return this.lastDeflection; }
  get color(): THREE.Color { return this.surface.material.color; }
  set color(value: THREE.ColorRepresentation) {
    this.checkActive();
    this.surface.material.color.set(value);
    this.surface.material.needsUpdate = true;
  }
  get outline(): boolean { return this.featureOutline.visible; }
  set outline(value: boolean) {
    this.checkActive();
    this.featureOutline.visible = value;
    this.surface.material.polygonOffset = value;
    this.surface.material.polygonOffsetFactor = 1;
    this.surface.material.polygonOffsetUnits = 1;
  }

  constructor(options: AnalyticPrimitiveOptions | AnalyticBrepOptions) {
    super();
    const id = options.kind === "brep" ? "" : options.ogid ?? getUUID();
    this.kernel = options.kind === "brep"
      ? new OGAnalyticBrep(options.serialized)
      : createPrimitiveKernel(options, id);
    this.ogid = this.kernel.get_id();
    this.surface = new THREE.Mesh(
      new THREE.BufferGeometry(),
      new THREE.MeshStandardMaterial({ color: options.color ?? 0x6699dd }),
    );
    this.surface.userData.ogid = this.ogid;
    this.featureOutline = new THREE.LineSegments(new THREE.BufferGeometry(), new THREE.LineBasicMaterial({ color: 0x243040 }));
    this.featureOutline.visible = false;
    this.featureOutline.userData.ogid = this.ogid;
    this.add(this.surface, this.featureOutline);
    try {
      this.retessellate(options.deflection ?? (options.kind === "brep" ? 0.001 : options.accuracy?.tessellation ?? 0.001));
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  static fromBrep(serialized: string, options: Omit<AnalyticBrepOptions, "kind" | "serialized"> = {}): AnalyticSolid {
    return new AnalyticSolid({ ...options, kind: "brep", serialized });
  }

  boolean(other: AnalyticSolid, operation: AnalyticBooleanOp, options: { deflection?: number; color?: THREE.ColorRepresentation } = {}): AnalyticSolid {
    this.checkActive();
    other.checkActive();
    const host = this.worldKernel();
    let cutter: OGAnalyticBrep | undefined;
    let result: OGAnalyticBrep | undefined;
    try {
      cutter = other.worldKernel();
      result = host.boolean_brep(cutter, operation, getUUID());
      const report = result.get_boolean_report_serialized();
      const solid = AnalyticSolid.fromBrep(result.get_brep_serialized(), {
        deflection: options.deflection ?? this.lastDeflection,
        color: options.color ?? this.surface.material.color,
      });
      solid.booleanReport = report ? JSON.parse(report) as AnalyticBooleanReport : undefined;
      solid.name = this.name;
      solid.userData = { ...this.userData };
      solid.outline = this.outline;
      return solid;
    } catch (error) {
      throw parseAnalyticGeometryError(error);
    } finally {
      result?.free();
      cutter?.free();
      host.free();
    }
  }

  union(other: AnalyticSolid, options: { deflection?: number; color?: THREE.ColorRepresentation } = {}): AnalyticSolid {
    return this.boolean(other, "union", options);
  }

  intersection(other: AnalyticSolid, options: { deflection?: number; color?: THREE.ColorRepresentation } = {}): AnalyticSolid {
    return this.boolean(other, "intersection", options);
  }

  subtract(cutters: AnalyticSolid[], options: { deflection?: number; color?: THREE.ColorRepresentation } = {}): AnalyticSolid {
    if (!Array.isArray(cutters) || cutters.length === 0 || cutters.some((cutter) => !(cutter instanceof AnalyticSolid))) {
      throw new Error("Analytic subtraction requires a nonempty AnalyticSolid array");
    }
    if (cutters.length > 1) {
      const host = this.worldKernel();
      const cutterKernels: OGAnalyticBrep[] = [];
      let result: OGAnalyticBrep | undefined;
      try {
        for (const cutter of cutters) cutterKernels.push(cutter.worldKernel());
        const payload = `[${cutterKernels.map((cutter) => cutter.get_brep_serialized()).join(",")}]`;
        result = host.subtract_planar_cutters(payload, getUUID());
        const report = result.get_boolean_report_serialized();
        const solid = AnalyticSolid.fromBrep(result.get_brep_serialized(), {
          deflection: options.deflection ?? this.lastDeflection,
          color: options.color ?? this.surface.material.color,
        });
        solid.booleanReport = report ? JSON.parse(report) as AnalyticBooleanReport : undefined;
        solid.name = this.name;
        solid.userData = { ...this.userData };
        solid.outline = this.outline;
        return solid;
      } catch (error) {
        const parsed = parseAnalyticGeometryError(error);
        // The native mixed-cutter path already chose a safe cutter order and
        // checked spatial overlap. A serial fallback may drop an earlier
        // cylindrical void or accept an invalid arched profile.
        if (parsed.code !== "coverage_gap"
          || parsed.families?.[0]?.includes("mixed cutter batch")) throw parsed;
      } finally {
        result?.free();
        cutterKernels.forEach((cutter) => cutter.free());
        host.free();
      }
    }
    let result: AnalyticSolid | undefined;
    try {
      for (const cutter of cutters) {
        const next = (result ?? this).boolean(cutter, "subtraction", options);
        result?.dispose();
        result = next;
      }
      return result!;
    } catch (error) {
      result?.dispose();
      throw error;
    }
  }

  shell(thickness: number, options: { deflection?: number; color?: THREE.ColorRepresentation } = {}): AnalyticSolid {
    this.checkActive();
    if (!Number.isFinite(thickness) || thickness <= 0) throw new Error("Shell thickness must be finite and positive");
    const host = this.worldKernel();
    let result: OGAnalyticBrep | undefined;
    try {
      result = host.shell(thickness, getUUID());
      const report = result.get_boolean_report_serialized();
      const solid = AnalyticSolid.fromBrep(result.get_brep_serialized(), {
        deflection: options.deflection ?? this.lastDeflection,
        color: options.color ?? this.surface.material.color,
      });
      solid.booleanReport = report ? JSON.parse(report) as AnalyticBooleanReport : undefined;
      solid.name = this.name;
      solid.userData = { ...this.userData };
      solid.outline = this.outline;
      return solid;
    } catch (error) {
      throw parseAnalyticGeometryError(error);
    } finally {
      result?.free();
      host.free();
    }
  }

  getBrepSerialized(): string {
    this.checkActive();
    return this.kernel.get_brep_serialized();
  }

  getBrepData(): Record<string, unknown> {
    return JSON.parse(this.getBrepSerialized()) as Record<string, unknown>;
  }

  /** Rebuilds authored analytic geometry while preserving the wrapper and placement. */
  protected replaceDefinition(options: AnalyticPrimitiveOptions): AnalyticTessellationStats {
    this.checkActive();
    const next = createPrimitiveKernel(options, this.ogid);
    const previous = this.kernel;
    this.kernel = next;
    try {
      const stats = this.retessellate(options.deflection ?? this.lastDeflection);
      previous.free();
      this.booleanReport = undefined;
      return stats;
    } catch (error) {
      this.kernel = previous;
      next.free();
      throw error;
    }
  }

  getWorldBrepSerialized(): string {
    const placed = this.worldKernel();
    try { return placed.get_brep_serialized(); }
    finally { placed.free(); }
  }

  prepareIfcExchange(): AnalyticExchangeBodyV2 {
    const placed = this.worldKernel();
    try { return JSON.parse(placed.prepare_ifc_exchange()) as AnalyticExchangeBodyV2; }
    finally { placed.free(); }
  }

  exportIfc(): { text: string; report: AnalyticExchangeBodyV2 } {
    const report = this.prepareIfcExchange();
    return { text: analyticIfcText(report, this.ogid), report };
  }

  /** Exports world placement and authored metre geometry, converted once to the selected STEP unit. */
  exportStep(lengthUnit: "metre" | "millimetre" = "metre"): AnalyticStepExport {
    const placed = this.worldKernel();
    try { return JSON.parse(placed.export_step(lengthUnit)) as AnalyticStepExport; }
    finally { placed.free(); }
  }

  /** Exports a binary STL tessellated from the placed analytic BRep at an explicit error budget. */
  exportStl(deflection = this.lastDeflection): AnalyticStlExport {
    if (!Number.isFinite(deflection) || deflection <= 0) {
      throw new Error("STL deflection must be finite and positive");
    }
    const placed = this.worldKernel();
    const result = tessellate_brep(
      placed.get_brep_serialized(),
      deflection * GEOMETRY_DEFLECTION_SHARE,
    );
    try {
      const positions = result.positions();
      const indices = result.indices();
      const triangleCount = indices.length / 3;
      if (!Number.isSafeInteger(triangleCount) || triangleCount > 0xffffffff) {
        throw new Error("STL triangle count exceeds the binary format limit");
      }
      const bytes = new Uint8Array(84 + triangleCount * 50);
      const header = "OpenGeometry BRep v2 analytic tessellation";
      for (let index = 0; index < Math.min(header.length, 80); index += 1) {
        bytes[index] = header.charCodeAt(index);
      }
      const view = new DataView(bytes.buffer);
      view.setUint32(80, triangleCount, true);
      let encodingError = 0;
      const writeFloat = (offset: number, value: number): void => {
        view.setFloat32(offset, value, true);
        encodingError = Math.max(encodingError, Math.abs(value - view.getFloat32(offset, true)));
      };
      for (let triangle = 0; triangle < triangleCount; triangle += 1) {
        const ids = [indices[triangle * 3], indices[triangle * 3 + 1], indices[triangle * 3 + 2]];
        const points = ids.map((id) => [
          positions[id * 3], positions[id * 3 + 1], positions[id * 3 + 2],
        ] as [number, number, number]);
        const ab = points[1].map((value, axis) => value - points[0][axis]);
        const ac = points[2].map((value, axis) => value - points[0][axis]);
        const cross = [
          ab[1] * ac[2] - ab[2] * ac[1],
          ab[2] * ac[0] - ab[0] * ac[2],
          ab[0] * ac[1] - ab[1] * ac[0],
        ];
        const length = Math.hypot(...cross);
        if (!Number.isFinite(length) || length === 0) {
          throw new Error("Analytic tessellation produced a degenerate STL triangle");
        }
        const offset = 84 + triangle * 50;
        cross.forEach((value, axis) => writeFloat(offset + axis * 4, value / length));
        points.forEach((point, vertex) => point.forEach((value, axis) => {
          writeFloat(offset + 12 + (vertex * 3 + axis) * 4, value);
        }));
        view.setUint16(offset + 48, 0, true);
      }
      const achievedDeflection = result.achieved_deflection() + encodingError;
      if (achievedDeflection > deflection) {
        throw new Error("Binary STL precision cannot represent the requested deflection");
      }
      return {
        bytes,
        report: {
          schema_version: 2,
          revision: result.revision().toString(),
          triangle_count: triangleCount,
          requested_deflection: deflection,
          achieved_deflection: achievedDeflection,
          float32_encoding_error: encodingError,
          quality: "tessellated_from_analytic_brep",
        },
      };
    } finally {
      result.free();
      placed.free();
    }
  }

  faceIdForTriangle(triangle: number): number | undefined {
    if (!Number.isInteger(triangle) || triangle < 0) return undefined;
    return this.triangleFaces[triangle];
  }

  edgeIdForOutlineSegment(segment: number): number | undefined {
    if (!Number.isInteger(segment) || segment < 0) return undefined;
    return this.outlineEdges[segment];
  }

  /** Evaluates the oriented support normal at a projected model-space point; this does not classify trim membership. */
  normalAtFace(faceId: number, modelPoint: THREE.Vector3): THREE.Vector3 {
    this.checkActive();
    if (!Number.isInteger(faceId) || faceId < 0 || faceId > 0xffffffff) throw new Error("Face ID must be an unsigned integer");
    const normal = this.kernel.face_normal_at(faceId, new Float64Array(modelPoint.toArray()));
    return new THREE.Vector3(normal[0], normal[1], normal[2]);
  }

  /** Classifies a world-space point against the regularized material represented by this solid. */
  classifyPoint(worldPoint: THREE.Vector3): AnalyticPointClassification {
    if (![worldPoint.x, worldPoint.y, worldPoint.z].every(Number.isFinite)) {
      throw new Error("Point coordinates must be finite");
    }
    const placed = this.worldKernel();
    try {
      return placed.classify_point(new Float64Array(worldPoint.toArray())) as AnalyticPointClassification;
    } finally {
      placed.free();
    }
  }

  projectTo2DLines(
    camera: AnalyticProjectionCamera,
    hlr: AnalyticProjectionHlr,
    deflection = this.lastDeflection,
  ): string {
    if (!Number.isFinite(deflection) || deflection <= 0) {
      throw new Error("Projection deflection must be finite and positive");
    }
    const placed = this.worldKernel();
    try {
      return placed.project_to_2d_lines(JSON.stringify(camera), JSON.stringify(hlr), deflection);
    } finally {
      placed.free();
    }
  }

  getModelBounds(): THREE.Box3 {
    this.checkActive();
    const bounds = this.kernel.get_bounds();
    if (bounds.length === 0) return new THREE.Box3();
    return new THREE.Box3(
      new THREE.Vector3(bounds[0], bounds[1], bounds[2]),
      new THREE.Vector3(bounds[3], bounds[4], bounds[5]),
    );
  }

  retessellate(deflection: number): AnalyticTessellationStats {
    this.checkActive();
    return this.applyTessellation(this.tessellate(deflection * GEOMETRY_DEFLECTION_SHARE), deflection);
  }

  tessellate(deflection: number, options: AnalyticTessellationOptions = {}): AnalyticTessellationData {
    this.checkActive();
    if (!Number.isFinite(deflection) || deflection <= 0) throw new Error("Deflection must be finite and positive");
    if (options.maxTriangles !== undefined
      && (!Number.isSafeInteger(options.maxTriangles) || options.maxTriangles <= 0 || options.maxTriangles > 2_000_000)) {
      throw new Error("maxTriangles must be an integer between 1 and 2000000");
    }
    const result = tessellate_brep(
      this.kernel.get_brep_serialized(),
      deflection,
      options.maxTriangles === undefined ? undefined : JSON.stringify({ max_triangles: options.maxTriangles }),
    );
    try {
      return {
        positions: result.positions(), normals: result.normals(), indices: result.indices(),
        triangleFaceIds: result.triangle_face_ids(), outlinePositions: result.outline_positions(),
        outlineEdgeIds: result.outline_edge_ids(), revision: result.revision(), achievedDeflection: result.achieved_deflection(),
      };
    } finally {
      result.free();
    }
  }

  /** Applies a transient worker mesh only when it still matches this authored model revision. */
  applyTessellation(result: AnalyticTessellationData, deflection: number): AnalyticTessellationStats {
      this.checkActive();
      if (!Number.isFinite(deflection) || deflection <= 0 || !Number.isFinite(result.achievedDeflection)
        || result.achievedDeflection < 0 || result.achievedDeflection > deflection || result.revision !== this.modelRevision) {
        throw new Error("Tessellation does not match the model revision or requested deflection");
      }
      const positions = result.positions;
      const indices = result.indices;
      const faceIds = result.triangleFaceIds;
      const outline = result.outlinePositions;
      const edgeIds = result.outlineEdgeIds;
      if (!(positions instanceof Float64Array) || !(result.normals instanceof Float32Array)
        || !(indices instanceof Uint32Array) || !(faceIds instanceof Uint32Array)
        || positions.length % 3 || result.normals.length !== positions.length || indices.length % 3
        || indices.length > 6_000_000 || faceIds.length !== indices.length / 3
        || !(outline instanceof Float64Array) || !(edgeIds instanceof Uint32Array)
        || outline.length % 6 || outline.length > 12_000_000 || edgeIds.length !== outline.length / 6) {
        throw new Error("Invalid or oversized tessellation buffers");
      }
      const faceCount = this.kernel.get_face_count();
      const edgeCount = this.kernel.get_edge_count();
      if (indices.some((index) => index >= positions.length / 3) || faceIds.some((id) => id >= faceCount)
        || positions.some((value) => !Number.isFinite(value)) || result.normals.some((value) => !Number.isFinite(value))
        || outline.some((value) => !Number.isFinite(value)) || edgeIds.some((id) => id >= edgeCount)) {
        throw new Error("Invalid tessellation coordinates or topology mappings");
      }
      const min = [Infinity, Infinity, Infinity];
      const max = [-Infinity, -Infinity, -Infinity];
      for (let i = 0; i < positions.length; i++) {
        min[i % 3] = Math.min(min[i % 3], positions[i]);
        max[i % 3] = Math.max(max[i % 3], positions[i]);
      }
      const origin = positions.length
        ? min.map((value, i) => value / 2 + max[i] / 2)
        : [0, 0, 0];
      let encodingError = 0;
      const encode = (coordinates: Float64Array): Float32Array => {
        const encoded = new Float32Array(coordinates.length);
        for (let i = 0; i < coordinates.length; i += 3) {
          const errors = [0, 0, 0];
          for (let j = 0; j < 3; j++) {
            const local = coordinates[i + j] - origin[j];
            encoded[i + j] = local;
            errors[j] = Math.abs(local - encoded[i + j]);
          }
          encodingError = Math.max(encodingError, Math.hypot(...errors));
        }
        return encoded;
      };
      const gpuPositions = encode(positions);
      const gpuOutline = encode(outline);
      if (!Number.isFinite(encodingError) || encodingError > deflection / 4) {
        throw new Error("GPU coordinate precision cannot represent the requested deflection");
      }
      if (result.achievedDeflection + encodingError > deflection) {
        throw new Error("Combined tessellation and GPU encoding error exceeds requested deflection");
      }
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(gpuPositions, 3));
      geometry.setAttribute("normal", new THREE.BufferAttribute(result.normals, 3));
      geometry.setIndex(new THREE.BufferAttribute(indices, 1));
      geometry.computeBoundingBox();
      geometry.computeBoundingSphere();
      const outlineGeometry = new THREE.BufferGeometry();
      outlineGeometry.setAttribute("position", new THREE.BufferAttribute(gpuOutline, 3));
      if (gpuOutline.length) { outlineGeometry.computeBoundingBox(); outlineGeometry.computeBoundingSphere(); }
      const previous = this.surface.geometry;
      const previousOutline = this.featureOutline.geometry;
      this.surface.geometry = geometry;
      this.surface.position.set(origin[0], origin[1], origin[2]);
      this.featureOutline.geometry = outlineGeometry;
      this.featureOutline.position.copy(this.surface.position);
      this.triangleFaces = faceIds;
      this.outlineEdges = edgeIds;
      previous.dispose();
      previousOutline.dispose();
      this.lastDeflection = deflection;
      return {
        requestedDeflection: deflection,
        achievedDeflection: result.achievedDeflection + encodingError,
        triangleCount: indices.length / 3,
        faceCount,
        revision: result.revision,
      };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.surface.geometry.dispose();
    this.surface.material.dispose();
    this.featureOutline.geometry.dispose();
    this.featureOutline.material.dispose();
    this.kernel.free();
  }

  private checkActive(): void {
    if (this.disposed) throw new Error("AnalyticSolid has been disposed");
  }

  private worldKernel(): OGAnalyticBrep {
    this.checkActive();
    this.updateWorldMatrix(true, false);
    const elements = this.matrixWorld.elements;
    if (elements.every((value, index) => value === (index % 5 === 0 ? 1 : 0))) {
      return new OGAnalyticBrep(this.kernel.get_brep_serialized());
    }
    const { frame, scale } = matrixToWorldTransform(this.matrixWorld);
    return this.kernel.placed(JSON.stringify(frame), scale);
  }
}

function createPrimitiveKernel(options: AnalyticPrimitiveOptions, ogid: string): OGAnalyticBrep {
    const frame = options.frame ?? (options.kind === "planarPolyhedron" || options.kind === "polygonLoft"
      ? { origin: [0, 0, 0], x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] }
      : { origin: [0, 0, 0], x: [1, 0, 0], y: [0, 0, -1], z: [0, 1, 0] });
    const profileExtent = options.kind === "linearExtrusion"
      ? [options.outer, ...options.holes].reduce((extent, loop) => (
        loop.reduce((loopExtent, point) => Math.max(loopExtent, Math.abs(point[0]), Math.abs(point[1])), extent)
      ), 0)
      : options.kind === "arcEdgedExtrusion"
        ? [options.outer, ...(options.holes ?? [])].reduce((extent, ring) => ring.reduce((loopExtent, edge) => edge.kind === "line"
          ? Math.max(loopExtent, ...edge.from.map(Math.abs), ...edge.to.map(Math.abs))
          : Math.max(loopExtent, Math.abs(edge.center[0]) + edge.radius, Math.abs(edge.center[1]) + edge.radius), extent), 0)
      : 0;
    const loftExtent = options.kind === "polygonLoft"
      ? [...options.lower, ...options.upper].reduce((extent, point) => Math.max(extent, ...point.map(Math.abs)), 0)
      : 0;
    const polyhedronExtent = options.kind === "planarPolyhedron"
      ? options.vertices.reduce((extent, point) => Math.max(extent, Math.abs(point[0]), Math.abs(point[1]), Math.abs(point[2])), 0)
      : 0;
    const size = options.kind === "torus"
      ? 2 * (options.majorRadius + options.minorRadius)
      : options.kind === "coaxialCircleLoft"
        ? Math.max(2 * options.lowerRadius, 2 * options.upperRadius, options.height)
      : options.kind === "revolvedRectangle"
        ? Math.max(2 * options.outerRadius, options.height)
      : options.kind === "chamferedCuboid"
        ? Math.max(options.width, options.depth, options.height)
      : options.kind === "linearExtrusion" || options.kind === "arcEdgedExtrusion"
        ? Math.max(options.height, profileExtent)
      : options.kind === "polygonLoft"
        ? loftExtent
      : options.kind === "boxWithArchedOpening"
        ? Math.max(options.width, options.depth, options.height)
      : options.kind === "planarPolyhedron"
        ? polyhedronExtent
      : options.kind === "frustum"
        ? Math.max(2 * options.lowerRadius, 2 * options.upperRadius, options.height)
        : options.kind === "annularSectorExtrusion" || options.kind === "annularSectorExtrusionWithOpenings" || options.kind === "annularSectorExtrusionWithArchedOpening"
          ? Math.max(2 * options.radius + options.thickness, options.height)
        : options.kind === "cuboid"
          ? Math.max(options.width, options.depth, options.height)
        : options.kind === "sphere"
          ? 2 * options.radius
          : Math.max(2 * options.radius, options.height);
    const coordinateFloor = Math.max(...frame.origin.map(Math.abs)) * 64 * Number.EPSILON;
    const geometric = Math.max(1e-9, size * 1e-8, coordinateFloor);
    const accuracy = options.accuracy ?? {
      geometric, intersection: geometric / 4, tessellation: 0.001, exchange: 0.00001,
    };
    const dimensions = options.kind === "torus"
      ? { major_radius: options.majorRadius, minor_radius: options.minorRadius }
      : options.kind === "coaxialCircleLoft"
        ? { lower_radius: options.lowerRadius, upper_radius: options.upperRadius, height: options.height }
      : options.kind === "revolvedRectangle"
        ? { inner_radius: options.innerRadius, outer_radius: options.outerRadius, height: options.height }
      : options.kind === "chamferedCuboid"
        ? { size: [options.width, options.depth, options.height], chamfer: options.chamfer }
      : options.kind === "filletedCylinder"
        ? { radius: options.radius, height: options.height, fillet_radius: options.filletRadius }
      : options.kind === "linearExtrusion"
        ? { outer: options.outer, holes: options.holes, height: options.height }
      : options.kind === "arcEdgedExtrusion"
        ? { outer: options.outer.map((edge) => edge.kind === "line" ? edge : {
            kind: "arc", center: edge.center, radius: edge.radius,
            start_angle: edge.startAngle, sweep_angle: edge.sweepAngle,
          }), holes: (options.holes ?? []).map((ring) => ring.map((edge) => edge.kind === "line" ? edge : {
            kind: "arc", center: edge.center, radius: edge.radius,
            start_angle: edge.startAngle, sweep_angle: edge.sweepAngle,
          })), height: options.height }
      : options.kind === "polygonLoft"
        ? {
            lower: options.lower,
            upper: options.upper,
            alignment: options.alignment === undefined || options.alignment === "auto"
              ? { kind: "auto" }
              : {
                  kind: "indexed",
                  upper_start: options.alignment.upperStartIndex,
                  reverse_upper: options.alignment.reverseUpper ?? false,
                },
          }
      : options.kind === "boxWithArchedOpening"
        ? { width: options.width, depth: options.depth, height: options.height, opening: options.opening }
      : options.kind === "planarPolyhedron"
        ? { vertices: options.vertices, faces: options.faces }
      : options.kind === "frustum"
        ? { lower_radius: options.lowerRadius, upper_radius: options.upperRadius, height: options.height }
        : options.kind === "annularSectorExtrusion" || options.kind === "annularSectorExtrusionWithOpenings" || options.kind === "annularSectorExtrusionWithArchedOpening"
          ? { radius: options.radius, thickness: options.thickness, height: options.height, start_angle: options.startAngle, sweep_angle: options.sweepAngle, ...(options.kind === "annularSectorExtrusionWithOpenings" ? { openings: options.openings.map((opening) => ({ id: opening.id, angle: opening.angle, width: opening.width, bottom: opening.bottom, height: opening.height })) } : options.kind === "annularSectorExtrusionWithArchedOpening" ? { opening: { id: options.opening.id, angle: options.opening.angle, width: options.opening.width, bottom: options.opening.bottom, height: options.opening.height } } : {}) }
        : options.kind === "cylinderSector"
          ? { radius: options.radius, height: options.height, start_angle: options.startAngle, sweep_angle: options.sweepAngle }
        : options.kind === "cuboid"
          ? { size: [options.width, options.depth, options.height] }
        : options.kind === "sphere"
          ? { radius: options.radius }
          : { radius: options.radius, height: options.height };
    return OGAnalyticBrep.from_primitive(JSON.stringify({
      kind: options.kind === "annularSectorExtrusion"
        ? "annular_sector_extrusion"
        : options.kind === "annularSectorExtrusionWithOpenings"
          ? "annular_sector_extrusion_with_openings"
        : options.kind === "annularSectorExtrusionWithArchedOpening"
          ? "annular_sector_extrusion_with_arched_opening"
        : options.kind === "coaxialCircleLoft"
          ? "coaxial_circle_loft"
        : options.kind === "revolvedRectangle"
          ? "revolved_rectangle"
        : options.kind === "chamferedCuboid"
          ? "chamfered_cuboid"
        : options.kind === "filletedCylinder"
          ? "filleted_cylinder"
        : options.kind === "linearExtrusion"
          ? "linear_extrusion"
        : options.kind === "arcEdgedExtrusion"
          ? "arc_edged_extrusion"
        : options.kind === "polygonLoft"
          ? "polygon_loft"
        : options.kind === "boxWithArchedOpening"
          ? "box_with_arched_opening"
        : options.kind === "planarPolyhedron"
          ? "planar_polyhedron"
        : options.kind === "cylinderSector"
          ? "cylinder_sector"
          : options.kind,
      id: ogid, frame, accuracy, ...dimensions,
    }));
}
