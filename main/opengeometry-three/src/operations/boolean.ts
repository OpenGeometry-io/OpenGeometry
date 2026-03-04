import { OGBoolean } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

export const BooleanOperation = {
  Union: "union",
  Intersection: "intersection",
  Difference: "difference",
} as const;

export type BooleanOperationKind =
  (typeof BooleanOperation)[keyof typeof BooleanOperation];

export function parseBooleanOperation(value: string): BooleanOperationKind {
  switch (value) {
    case BooleanOperation.Intersection:
      return BooleanOperation.Intersection;
    case BooleanOperation.Difference:
      return BooleanOperation.Difference;
    case BooleanOperation.Union:
    default:
      return BooleanOperation.Union;
  }
}

export interface BooleanConstraints {
  epsilon?: number;
  snap?: number;
}

export class BooleanShape extends THREE.Mesh {
  private readonly kernelBoolean = new OGBoolean();
  #outlineMesh: THREE.LineSegments | null = null;

  constructor(
    left: THREE.Mesh,
    right: THREE.Mesh,
    operation: BooleanOperationKind,
    constraints?: BooleanConstraints,
    material?: THREE.Material
  ) {
    const { geometry, outline } = BooleanShape.computeGeometry(
      left,
      right,
      operation,
      constraints
    );

    super(
      geometry,
      material ??
        new THREE.MeshStandardMaterial({
          color: 0x3b82f6,
          transparent: true,
          opacity: 0.7,
        })
    );

    this.applyOutline(outline);
  }

  run(
    left: THREE.Mesh,
    right: THREE.Mesh,
    operation: BooleanOperationKind,
    constraints?: BooleanConstraints
  ) {
    this.geometry.dispose();
    const { geometry, outline } = BooleanShape.computeGeometry(
      left,
      right,
      operation,
      constraints,
      this.kernelBoolean
    );
    this.geometry = geometry;
    this.geometry.computeVertexNormals();
    this.applyOutline(outline);
  }

  set outline(enable: boolean) {
    if (!enable) {
      if (this.#outlineMesh) {
        this.remove(this.#outlineMesh);
        this.#outlineMesh.geometry.dispose();
        (this.#outlineMesh.material as THREE.Material).dispose();
        this.#outlineMesh = null;
      }
      return;
    }

    if (this.#outlineMesh) {
      this.add(this.#outlineMesh);
    }
  }

  private applyOutline(outlinePositions: number[]) {
    if (this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      (this.#outlineMesh.material as THREE.Material).dispose();
      this.#outlineMesh = null;
    }

    if (outlinePositions.length === 0) {
      return;
    }

    const outlineGeometry = new THREE.BufferGeometry();
    outlineGeometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(outlinePositions, 3)
    );
    const outlineMaterial = new THREE.LineBasicMaterial({ color: 0x111827 });
    this.#outlineMesh = new THREE.LineSegments(outlineGeometry, outlineMaterial);
    this.add(this.#outlineMesh);
  }

  static computeGeometry(
    left: THREE.Mesh,
    right: THREE.Mesh,
    operation: BooleanOperationKind,
    constraints?: BooleanConstraints,
    kernelBoolean = new OGBoolean()
  ) {
    const leftBuffer = extractWorldSpaceTriangleBuffer(left);
    const rightBuffer = extractWorldSpaceTriangleBuffer(right);

    const result = kernelBoolean.compute(
      JSON.stringify(leftBuffer),
      JSON.stringify(rightBuffer),
      operation,
      constraints ? JSON.stringify(constraints) : undefined
    );

    const outlineResult = kernelBoolean.get_outline_geometry_serialized();

    const positions = JSON.parse(result) as number[];
    const outline = JSON.parse(outlineResult) as number[];
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(positions, 3)
    );
    geometry.computeVertexNormals();

    return { geometry, outline };
  }
}

function extractWorldSpaceTriangleBuffer(mesh: THREE.Mesh): number[] {
  const geometry = mesh.geometry;
  if (!(geometry instanceof THREE.BufferGeometry)) {
    throw new Error("Boolean operations require THREE.BufferGeometry meshes.");
  }

  const position = geometry.getAttribute("position");
  if (!position) {
    throw new Error("Boolean operations require a position attribute.");
  }

  mesh.updateWorldMatrix(true, false);
  const world = mesh.matrixWorld;

  const source = geometry.toNonIndexed();
  const sourcePosition = source.getAttribute("position");
  const out: number[] = [];
  const v = new THREE.Vector3();

  for (let i = 0; i < sourcePosition.count; i++) {
    v.fromBufferAttribute(sourcePosition, i).applyMatrix4(world);
    out.push(v.x, v.y, v.z);
  }

  source.dispose();
  return out;
}
