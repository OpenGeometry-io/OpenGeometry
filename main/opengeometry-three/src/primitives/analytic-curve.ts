import * as THREE from "three";
import { OGAnalyticBrep } from "../../../opengeometry/pkg/opengeometry";
import type { AnalyticAccuracy, AnalyticFrame } from "../shapes/analytic-solid";
import { getUUID } from "../utils/randomizer";
import { GEOMETRY_DEFLECTION_SHARE } from "../rendering/analytic-tessellation";

export type AnalyticCurveOptions = {
  ogid?: string;
  frame?: AnalyticFrame;
  accuracy?: AnalyticAccuracy;
  startAngle: number;
  sweepAngle: number;
  deflection?: number;
  color?: THREE.ColorRepresentation;
} & (
  | { kind: "arc"; radius: number }
  | { kind: "ellipticalArc"; majorRadius: number; minorRadius: number }
);

export interface AnalyticCurveStats {
  requestedDeflection: number;
  achievedDeflection: number;
  segmentCount: number;
  revision: bigint;
}

export class AnalyticCurve extends THREE.Group {
  readonly ogid: string;
  readonly line: THREE.LineSegments<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  protected kernel: OGAnalyticBrep;
  private segmentEdges = new Uint32Array(0);
  private disposed = false;

  get color(): THREE.Color { return this.line.material.color; }
  set color(value: THREE.ColorRepresentation) {
    this.checkActive();
    this.line.material.color.set(value);
    this.line.material.needsUpdate = true;
  }

  constructor(options: AnalyticCurveOptions) {
    super();
    this.ogid = options.ogid ?? getUUID();
    const frame = options.frame ?? {
      origin: [0, 0, 0], x: [1, 0, 0], y: [0, 0, -1], z: [0, 1, 0],
    };
    const radius = options.kind === "arc" ? options.radius : options.majorRadius;
    const geometric = Math.max(1e-9, 2 * radius * 1e-8, ...frame.origin.map(value => Math.abs(value) * 64 * Number.EPSILON));
    const accuracy = options.accuracy ?? { geometric, intersection: geometric / 4, tessellation: 0.001, exchange: 0.00001 };
    this.kernel = createKernel(options, this.ogid, frame, accuracy);
    this.line = new THREE.LineSegments(new THREE.BufferGeometry(), new THREE.LineBasicMaterial({ color: options.color ?? 0x2563eb }));
    this.line.userData.ogid = this.ogid;
    this.add(this.line);
    try { this.retessellate(options.deflection ?? accuracy.tessellation); }
    catch (error) { this.dispose(); throw error; }
  }

  getBrepSerialized(): string {
    this.checkActive();
    return this.kernel.get_brep_serialized();
  }

  getBrepData(): Record<string, unknown> {
    return JSON.parse(this.getBrepSerialized()) as Record<string, unknown>;
  }

  protected replaceDefinition(options: AnalyticCurveOptions): AnalyticCurveStats {
    this.checkActive();
    const frame = options.frame ?? {
      origin: [0, 0, 0], x: [1, 0, 0], y: [0, 0, -1], z: [0, 1, 0],
    };
    const radius = options.kind === "arc" ? options.radius : options.majorRadius;
    const geometric = Math.max(1e-9, 2 * radius * 1e-8, ...frame.origin.map(value => Math.abs(value) * 64 * Number.EPSILON));
    const accuracy = options.accuracy ?? { geometric, intersection: geometric / 4, tessellation: 0.001, exchange: 0.00001 };
    const next = createKernel(options, this.ogid, frame, accuracy);
    const previous = this.kernel;
    this.kernel = next;
    try {
      const statistics = this.retessellate(options.deflection ?? accuracy.tessellation);
      if (options.color !== undefined) this.color = options.color;
      previous.free();
      return statistics;
    } catch (error) {
      this.kernel = previous;
      next.free();
      throw error;
    }
  }

  edgeIdForSegment(segment: number): number | undefined {
    if (!Number.isInteger(segment) || segment < 0) return undefined;
    return this.segmentEdges[segment];
  }

  retessellate(deflection: number): AnalyticCurveStats {
    this.checkActive();
    if (!Number.isFinite(deflection) || deflection <= 0) throw new Error("Deflection must be finite and positive");
    const result = this.kernel.tessellate(deflection * GEOMETRY_DEFLECTION_SHARE);
    try {
      const bounds = this.kernel.get_bounds();
      const origin = [0, 1, 2].map(i => bounds[i] / 2 + bounds[i + 3] / 2);
      const positions = result.outline_positions();
      const packed = new Float32Array(positions.length);
      let encodingError = 0;
      for (let i = 0; i < positions.length; i += 3) {
        const errors = [0, 0, 0];
        for (let j = 0; j < 3; j++) {
          const local = positions[i + j] - origin[j]; packed[i + j] = local;
          errors[j] = Math.abs(local - packed[i + j]);
        }
        encodingError = Math.max(encodingError, Math.hypot(...errors));
      }
      if (!Number.isFinite(encodingError) || encodingError > deflection / 4) throw new Error("GPU coordinate precision cannot represent the requested deflection");
      const edgeIds = result.outline_edge_ids();
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(packed, 3));
      geometry.computeBoundingBox(); geometry.computeBoundingSphere();
      const previous = this.line.geometry;
      this.line.geometry = geometry; this.line.position.set(origin[0], origin[1], origin[2]);
      this.segmentEdges = edgeIds; previous.dispose();
      return { requestedDeflection: deflection, achievedDeflection: result.achieved_deflection() + encodingError, segmentCount: edgeIds.length, revision: result.revision() };
    } finally { result.free(); }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.line.geometry.dispose(); this.line.material.dispose(); this.kernel.free();
  }
  private checkActive(): void { if (this.disposed) throw new Error("AnalyticCurve has been disposed"); }
}

function createKernel(
  options: AnalyticCurveOptions,
  id: string,
  frame: AnalyticFrame,
  accuracy: AnalyticAccuracy,
): OGAnalyticBrep {
  return OGAnalyticBrep.from_primitive(JSON.stringify({
    kind: options.kind === "arc" ? "arc" : "elliptical_arc", id, frame, accuracy,
    start_angle: options.startAngle, sweep_angle: options.sweepAngle,
    ...(options.kind === "arc"
      ? { radius: options.radius }
      : { major_radius: options.majorRadius, minor_radius: options.minorRadius }),
  }));
}
