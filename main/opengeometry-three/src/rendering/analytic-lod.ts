import * as THREE from "three";
import type { AnalyticSolid, AnalyticTessellationStats } from "../shapes/analytic-solid";
import { AnalyticTessellationWorker, GEOMETRY_DEFLECTION_SHARE } from "./analytic-tessellation";
import { getUUID } from "../utils/randomizer";

export function deflectionBucket(target: number, previous?: number): number {
  if (!Number.isFinite(target) || target <= 0) throw new Error("LOD target must be finite and positive");
  if (previous && target >= previous * 0.75 && target <= previous * 2.5) return previous;
  const bucket = 2 ** Math.floor(Math.log2(target));
  if (!Number.isFinite(bucket) || bucket <= 0) throw new Error("LOD target is outside the representable range");
  return bucket;
}

export function worldUnitsPerPixel(camera: THREE.PerspectiveCamera | THREE.OrthographicCamera, depth: number, physicalHeight: number): number {
  if (!Number.isFinite(physicalHeight) || physicalHeight <= 0) throw new Error("Physical viewport height must be positive");
  if (camera instanceof THREE.OrthographicCamera) return (camera.top - camera.bottom) / (camera.zoom * physicalHeight);
  if (!Number.isFinite(depth) || depth <= 0) throw new Error("Perspective depth must be positive");
  return 2 * depth * Math.tan(THREE.MathUtils.degToRad(camera.getEffectiveFOV()) / 2) / physicalHeight;
}

interface Entry {
  handle: string;
  solid: AnalyticSolid;
  bounds: THREE.Box3;
  ready: boolean;
  desired?: number;
  accepted?: number;
  generation: number;
  failed?: number;
}

export interface AnalyticLodOptions {
  fixedDeflection?: number;
  onUpdate?: (solid: AnalyticSolid, statistics: AnalyticTessellationStats) => void;
  onError?: (solid: AnalyticSolid, error: Error) => void;
}

/** Register authored changes explicitly; camera meshes never refresh a scene's authored BRep snapshots. */
export class AnalyticLodController {
  private readonly entries = new Map<AnalyticSolid, Entry>();
  private readonly cameraState = new Float64Array(34);
  private lastMotion = -Infinity;
  private lastSchedule = -Infinity;
  private initialized = false;
  private closed = false;
  private refining = false;
  private fixedDeflection?: number;

  constructor(private readonly worker: AnalyticTessellationWorker, private readonly options: AnalyticLodOptions = {}) {
    this.setDeflection(options.fixedDeflection);
  }

  setDeflection(deflection?: number): void {
    if (deflection !== undefined && (!Number.isFinite(deflection) || deflection <= 0)) throw new Error("Deflection must be finite and positive");
    this.fixedDeflection = deflection;
    for (const entry of this.entries.values()) {
      entry.generation++;
      entry.desired = entry.failed = undefined;
      this.worker.cancel(entry.handle);
    }
  }

  async register(solid: AnalyticSolid): Promise<void> {
    if (this.closed) throw new Error("LOD controller has been disposed");
    await this.unregister(solid);
    if (this.entries.size >= 64) throw new Error("LOD body limit exceeded");
    const entry: Entry = {
      handle: `lod-${getUUID()}`, solid, bounds: solid.getModelBounds(), ready: false, generation: 0,
    };
    this.entries.set(solid, entry);
    try {
      await this.worker.setModel(entry.handle, solid.getBrepSerialized());
      if (this.entries.get(solid) === entry) entry.ready = true;
      else await this.worker.removeModel(entry.handle);
    } catch (error) {
      if (this.entries.get(solid) === entry) this.entries.delete(solid);
      throw error;
    }
  }

  async unregister(solid: AnalyticSolid): Promise<void> {
    const entry = this.entries.get(solid);
    if (!entry) return;
    this.entries.delete(solid);
    entry.generation++;
    await this.worker.removeModel(entry.handle);
  }

  update(camera: THREE.PerspectiveCamera | THREE.OrthographicCamera, physicalHeight: number, now = performance.now(), motion = false): void {
    if (this.closed || this.refining) return;
    camera.updateMatrixWorld();
    let changed = !this.initialized || physicalHeight !== this.cameraState[32];
    for (let i = 0; i < 16; i++) {
      if (camera.matrixWorld.elements[i] !== this.cameraState[i] || camera.projectionMatrix.elements[i] !== this.cameraState[16 + i]) changed = true;
      this.cameraState[i] = camera.matrixWorld.elements[i];
      this.cameraState[16 + i] = camera.projectionMatrix.elements[i];
    }
    this.cameraState[32] = physicalHeight;
    if (this.initialized && (changed || motion)) this.lastMotion = now;
    this.initialized = true;
    const moving = now - this.lastMotion < 150;
    if (now - this.lastSchedule < 100) return;
    this.lastSchedule = now;
    const frustum = new THREE.Frustum().setFromProjectionMatrix(new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse));
    const candidates: { entry: Entry; deflection: number; distance: number }[] = [];
    for (const entry of this.entries.values()) {
      if (!entry.ready || entry.solid.isDisposed || !visible(entry.solid.surface) || !entry.solid.surface.layers.test(camera.layers) || entry.bounds.isEmpty()) continue;
      entry.solid.updateWorldMatrix(true, false);
      const bounds = entry.bounds.clone().applyMatrix4(entry.solid.matrixWorld);
      if (!frustum.intersectsBox(bounds)) continue;
      try {
        const scale = uniformScale(entry.solid.matrixWorld);
        const cameraBounds = bounds.clone().applyMatrix4(camera.matrixWorldInverse);
        const depth = Math.max(camera.near, -cameraBounds.max.z);
        const target = (this.fixedDeflection ?? worldUnitsPerPixel(camera, depth, physicalHeight) * (moving ? 2 : 0.5)) / scale;
        const deflection = this.fixedDeflection === undefined ? deflectionBucket(target, entry.desired) : target;
        if (entry.desired === deflection || entry.failed === deflection) continue;
        candidates.push({ entry, deflection, distance: depth });
      } catch (error) { this.options.onError?.(entry.solid, asError(error)); }
    }
    candidates.sort((a, b) => a.distance - b.distance);
    for (const { entry, deflection } of candidates) {
      entry.desired = deflection;
      const generation = ++entry.generation;
      const revision = entry.solid.modelRevision;
      void this.worker.tessellate(entry.handle, deflection * GEOMETRY_DEFLECTION_SHARE).then((mesh) => {
        if (this.closed || this.entries.get(entry.solid) !== entry || generation !== entry.generation || entry.solid.modelRevision !== revision) return;
        const stats = entry.solid.applyTessellation(mesh, deflection);
        entry.accepted = deflection;
        entry.failed = undefined;
        this.options.onUpdate?.(entry.solid, stats);
      }).catch((error) => {
        if (this.closed || this.entries.get(entry.solid) !== entry || generation !== entry.generation || error?.name === "AbortError") return;
        entry.failed = deflection;
        entry.desired = entry.accepted;
        this.options.onError?.(entry.solid, asError(error));
      });
    }
  }

  /** Explicit export refinement; a resource or precision failure rejects and retains each body's prior mesh. */
  async refine(deflection: number): Promise<void> {
    if (!Number.isFinite(deflection) || deflection <= 0) throw new Error("Deflection must be finite and positive");
    if (this.closed || this.refining) throw new Error("LOD controller is disposed or export refinement is already running");
    this.refining = true;
    try {
    for (const entry of [...this.entries.values()]) {
      if (!visible(entry.solid.surface)) continue;
      if (!entry.ready) throw new Error("Await model registration before export refinement");
      entry.solid.updateWorldMatrix(true, false);
      const local = deflection / uniformScale(entry.solid.matrixWorld);
      const generation = ++entry.generation;
      const mesh = await this.worker.tessellate(entry.handle, local * GEOMETRY_DEFLECTION_SHARE);
      if (this.closed || this.entries.get(entry.solid) !== entry || generation !== entry.generation) throw new DOMException("Export refinement superseded", "AbortError");
      const stats = entry.solid.applyTessellation(mesh, local);
      entry.desired = entry.accepted = local;
      entry.failed = undefined;
      this.options.onUpdate?.(entry.solid, stats);
    }
    } finally { this.refining = false; }
  }

  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    for (const entry of this.entries.values()) {
      entry.generation++;
      void this.worker.removeModel(entry.handle).catch((error) => {
        if (error?.name !== "AbortError") this.options.onError?.(entry.solid, asError(error));
      });
    }
    this.entries.clear();
  }
}

function visible(object: THREE.Object3D): boolean {
  for (let current: THREE.Object3D | null = object; current; current = current.parent) if (!current.visible) return false;
  return true;
}
function asError(error: unknown): Error { return error instanceof Error ? error : new Error(String(error)); }
export function uniformScale(matrix: THREE.Matrix4): number {
  const x = new THREE.Vector3().setFromMatrixColumn(matrix, 0);
  const y = new THREE.Vector3().setFromMatrixColumn(matrix, 1);
  const z = new THREE.Vector3().setFromMatrixColumn(matrix, 2);
  const scale = x.length();
  if (!Number.isFinite(scale) || scale <= 0 || Math.abs(y.length() / scale - 1) > 1e-9 || Math.abs(z.length() / scale - 1) > 1e-9
    || Math.abs(x.dot(y) / scale ** 2) > 1e-9 || Math.abs(x.dot(z) / scale ** 2) > 1e-9 || Math.abs(y.dot(z) / scale ** 2) > 1e-9
    || matrix.determinant() <= 0) throw new Error("Analytic placement requires positive uniform scale and a rigid frame");
  return scale;
}
