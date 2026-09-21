import * as THREE from "three";
import { AnalyticSolid } from "./analytic-solid";

export interface AnalyticPatternInstance {
  id?: string;
  matrix: THREE.Matrix4;
}

export interface AnalyticPatternHit {
  pattern: AnalyticPattern;
  instanceIndex: number;
  instanceId: string;
  source: AnalyticSolid;
  faceId?: number;
}

function finiteMatrix(matrix: THREE.Matrix4): boolean {
  return matrix.elements.every(Number.isFinite);
}

/**
 * Renders many rigid placements of one authoritative analytic body. The source
 * owns the BRep; instances own only identity and placement matrices.
 */
export class AnalyticPattern extends THREE.Group {
  readonly source: AnalyticSolid;
  readonly surface: THREE.InstancedMesh<THREE.BufferGeometry, THREE.Material>;
  readonly instanceIds: readonly string[];
  private disposed = false;

  constructor(source: AnalyticSolid, instances: readonly AnalyticPatternInstance[]) {
    super();
    if (!(source instanceof AnalyticSolid) || source.isDisposed) {
      throw new Error("Analytic patterns require an active AnalyticSolid source");
    }
    if (instances.length === 0 || instances.length > 100_000) {
      throw new Error("Analytic patterns require 1–100,000 instances");
    }
    if (instances.some(({ matrix }) => !(matrix instanceof THREE.Matrix4) || !finiteMatrix(matrix))) {
      throw new Error("Analytic pattern placements must be finite Matrix4 values");
    }
    const ids = instances.map((instance, index) => instance.id ?? `${source.ogid}:instance-${index}`);
    if (ids.some((id) => !id) || new Set(ids).size !== ids.length) {
      throw new Error("Analytic pattern instance identities must be nonempty and unique");
    }
    this.source = source;
    this.instanceIds = ids;
    this.surface = new THREE.InstancedMesh(
      source.surface.geometry,
      source.surface.material,
      instances.length,
    );
    this.surface.name = `${source.name || source.ogid} pattern`;
    this.surface.userData.ogid = source.ogid;
    this.surface.userData.analyticPattern = this;
    instances.forEach((instance, index) => this.surface.setMatrixAt(index, instance.matrix));
    this.surface.instanceMatrix.needsUpdate = true;
    this.add(this.surface);
  }

  get count(): number { return this.instanceIds.length; }

  setInstanceMatrix(index: number, matrix: THREE.Matrix4): void {
    this.checkActive();
    if (!Number.isInteger(index) || index < 0 || index >= this.count) {
      throw new Error("Analytic pattern instance index is outside the pattern");
    }
    if (!(matrix instanceof THREE.Matrix4) || !finiteMatrix(matrix)) {
      throw new Error("Analytic pattern placement must be a finite Matrix4");
    }
    this.surface.setMatrixAt(index, matrix);
    this.surface.instanceMatrix.needsUpdate = true;
    this.surface.computeBoundingBox();
    this.surface.computeBoundingSphere();
  }

  resolveIntersection(intersection: THREE.Intersection): AnalyticPatternHit | undefined {
    if (intersection.object !== this.surface || intersection.instanceId === undefined) return undefined;
    const instanceIndex = intersection.instanceId;
    if (instanceIndex < 0 || instanceIndex >= this.count) return undefined;
    const triangleIndex = intersection.faceIndex;
    const faceId = triangleIndex === undefined ? undefined : this.source.faceIdForTriangle(triangleIndex);
    return {
      pattern: this,
      instanceIndex,
      instanceId: this.instanceIds[instanceIndex],
      source: this.source,
      faceId,
    };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.remove(this.surface);
    this.surface.dispose();
  }

  private checkActive(): void {
    if (this.disposed) throw new Error("Analytic pattern has been disposed");
    if (this.source.isDisposed) throw new Error("Analytic pattern source has been disposed");
  }
}

export function linearPattern(
  source: AnalyticSolid,
  count: number,
  spacing: THREE.Vector3,
): AnalyticPattern {
  if (!Number.isInteger(count) || count < 1 || count > 100_000) {
    throw new Error("Linear pattern count must be an integer from 1 to 100,000");
  }
  if (!(spacing instanceof THREE.Vector3) || ![spacing.x, spacing.y, spacing.z].every(Number.isFinite)) {
    throw new Error("Linear pattern spacing must be a finite Vector3");
  }
  return new AnalyticPattern(source, Array.from({ length: count }, (_, index) => ({
    matrix: new THREE.Matrix4().makeTranslation(spacing.x * index, spacing.y * index, spacing.z * index),
  })));
}

export function rectangularPattern(
  source: AnalyticSolid,
  counts: readonly [number, number],
  spacing: readonly [THREE.Vector3, THREE.Vector3],
): AnalyticPattern {
  if (counts.some((count) => !Number.isInteger(count) || count < 1) || counts[0] * counts[1] > 100_000) {
    throw new Error("Rectangular pattern counts must be positive integers with at most 100,000 instances");
  }
  if (spacing.some((vector) => !(vector instanceof THREE.Vector3)
    || ![vector.x, vector.y, vector.z].every(Number.isFinite))) {
    throw new Error("Rectangular pattern spacing vectors must be finite");
  }
  const instances: AnalyticPatternInstance[] = [];
  for (let row = 0; row < counts[1]; row += 1) {
    for (let column = 0; column < counts[0]; column += 1) {
      instances.push({
        id: `${source.ogid}:instance-${column}-${row}`,
        matrix: new THREE.Matrix4().makeTranslation(
          spacing[0].x * column + spacing[1].x * row,
          spacing[0].y * column + spacing[1].y * row,
          spacing[0].z * column + spacing[1].z * row,
        ),
      });
    }
  }
  return new AnalyticPattern(source, instances);
}

export function circularPattern(
  source: AnalyticSolid,
  count: number,
  radius: number,
  options: { startAngle?: number; sweepAngle?: number; rotateInstances?: boolean } = {},
): AnalyticPattern {
  if (!Number.isInteger(count) || count < 1 || count > 100_000) {
    throw new Error("Circular pattern count must be an integer from 1 to 100,000");
  }
  if (!Number.isFinite(radius) || radius < 0) throw new Error("Circular pattern radius must be finite and nonnegative");
  const start = options.startAngle ?? 0;
  const sweep = options.sweepAngle ?? Math.PI * 2;
  if (![start, sweep].every(Number.isFinite) || sweep === 0) {
    throw new Error("Circular pattern angles must be finite with a nonzero sweep");
  }
  const closed = Math.abs(Math.abs(sweep) - Math.PI * 2) <= 64 * Number.EPSILON;
  const denominator = closed ? count : Math.max(1, count - 1);
  const instances = Array.from({ length: count }, (_, index) => {
    const angle = start + sweep * index / denominator;
    const matrix = new THREE.Matrix4().makeTranslation(radius * Math.cos(angle), 0, radius * Math.sin(angle));
    if (options.rotateInstances ?? true) matrix.multiply(new THREE.Matrix4().makeRotationY(-angle));
    return { id: `${source.ogid}:instance-${index}`, matrix };
  });
  return new AnalyticPattern(source, instances);
}
