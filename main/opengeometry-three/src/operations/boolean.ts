import { OGBoolean } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

export type BooleanOperation = "union" | "intersection" | "difference";

export interface BooleanConstraints {
  epsilon?: number;
  snap?: number;
}

export class BooleanShape extends THREE.Mesh {
  private readonly kernelBoolean = new OGBoolean();

  constructor(
    left: THREE.Mesh,
    right: THREE.Mesh,
    operation: BooleanOperation,
    constraints?: BooleanConstraints,
    material?: THREE.Material
  ) {
    const geometry = BooleanShape.computeGeometry(left, right, operation, constraints);
    super(
      geometry,
      material ??
        new THREE.MeshStandardMaterial({
          color: 0x3b82f6,
          transparent: true,
          opacity: 0.7,
        })
    );
  }

  run(
    left: THREE.Mesh,
    right: THREE.Mesh,
    operation: BooleanOperation,
    constraints?: BooleanConstraints
  ) {
    this.geometry.dispose();
    this.geometry = BooleanShape.computeGeometry(left, right, operation, constraints, this.kernelBoolean);
    this.geometry.computeVertexNormals();
  }

  static computeGeometry(
    left: THREE.Mesh,
    right: THREE.Mesh,
    operation: BooleanOperation,
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

    const positions = JSON.parse(result) as number[];
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
    geometry.computeVertexNormals();
    return geometry;
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
