import { BooleanOperation, OGBooleanResult } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

export type BooleanInputShape = {
  getBrepData: () => unknown;
};

export type BooleanKind = "union" | "intersection" | "difference";

export interface BooleanOptions {
  voxelSize?: number;
  color?: number;
  opacity?: number;
}

function toKernelOperation(operation: BooleanKind): BooleanOperation {
  switch (operation) {
    case "union":
      return BooleanOperation.Union;
    case "intersection":
      return BooleanOperation.Intersection;
    case "difference":
      return BooleanOperation.Difference;
  }
}

export class BooleanMesh extends THREE.Mesh {
  private readonly solver = new OGBooleanResult();

  constructor() {
    super(new THREE.BufferGeometry(), new THREE.MeshStandardMaterial({ color: 0x33aa33, transparent: true, opacity: 0.8 }));
  }

  compute(
    first: BooleanInputShape,
    second: BooleanInputShape,
    operation: BooleanKind,
    options?: BooleanOptions,
  ) {
    const voxelSize = options?.voxelSize ?? 0.15;

    this.solver.compute_from_brep_serialized(
      JSON.stringify(first.getBrepData()),
      JSON.stringify(second.getBrepData()),
      toKernelOperation(operation),
      voxelSize,
    );

    const geometry = new THREE.BufferGeometry();
    const geometryData = JSON.parse(this.solver.get_geometry_serialized());
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(geometryData, 3));
    geometry.computeVertexNormals();

    if (this.geometry) {
      this.geometry.dispose();
    }

    this.geometry = geometry;

    const color = options?.color ?? 0x33aa33;
    const opacity = options?.opacity ?? 0.8;
    this.material = new THREE.MeshStandardMaterial({
      color,
      transparent: opacity < 1,
      opacity,
    });
  }

  getBrepData() {
    return JSON.parse(this.solver.get_brep_serialized());
  }
}
