import * as THREE from "three";

export type BooleanOperationType = "union" | "subtract" | "intersect";

export interface BooleanOperationOptions {
  operation: BooleanOperationType;
  color?: number;
  opacity?: number;
  gridResolution?: number;
  constrainResultToPositiveY?: boolean;
}

interface BooleanField {
  min: THREE.Vector3;
  max: THREE.Vector3;
  step: THREE.Vector3;
  nx: number;
  ny: number;
  nz: number;
  cells: Uint8Array;
}

export class BooleanShape extends THREE.Mesh {
  readonly leftId: string;
  readonly rightId: string;
  readonly operation: BooleanOperationType;

  constructor(
    leftId: string,
    rightId: string,
    operation: BooleanOperationType,
    geometry: THREE.BufferGeometry,
    material: THREE.Material
  ) {
    super(geometry, material);
    this.leftId = leftId;
    this.rightId = rightId;
    this.operation = operation;
  }
}

function buildMaterial(options: BooleanOperationOptions): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({
    color: options.color ?? 0x3b82f6,
    metalness: 0,
    roughness: 0.7,
    transparent: true,
    opacity: options.opacity ?? 0.72,
    side: THREE.DoubleSide,
  });
}

function flattenedMesh(mesh: THREE.Mesh): THREE.Mesh {
  const geometry = mesh.geometry.clone();
  mesh.updateWorldMatrix(true, false);
  geometry.applyMatrix4(mesh.matrixWorld);
  geometry.computeBoundingBox();
  return new THREE.Mesh(geometry, new THREE.MeshBasicMaterial());
}

const raycaster = new THREE.Raycaster();
const direction = new THREE.Vector3(1, 0.1337, 0.017);

function isPointInsideMesh(point: THREE.Vector3, mesh: THREE.Mesh): boolean {
  raycaster.set(point, direction);
  const intersects = raycaster.intersectObject(mesh, false);
  return intersects.length % 2 === 1;
}

function combineBounds(left: THREE.Mesh, right: THREE.Mesh): { min: THREE.Vector3; max: THREE.Vector3 } {
  const leftBox = new THREE.Box3().setFromObject(left);
  const rightBox = new THREE.Box3().setFromObject(right);
  const min = leftBox.min.clone().min(rightBox.min);
  const max = leftBox.max.clone().max(rightBox.max);
  return { min, max };
}

function resultOccupancy(operation: BooleanOperationType, inLeft: boolean, inRight: boolean): boolean {
  switch (operation) {
    case "union":
      return inLeft || inRight;
    case "subtract":
      return inLeft && !inRight;
    case "intersect":
      return inLeft && inRight;
  }
}

function createBooleanField(left: THREE.Mesh, right: THREE.Mesh, options: BooleanOperationOptions): BooleanField {
  const resolution = Math.max(8, Math.min(64, Math.floor(options.gridResolution ?? 26)));
  const { min, max } = combineBounds(left, right);
  const size = max.clone().sub(min);

  const largest = Math.max(size.x, size.y, size.z, 1e-3);
  const nx = Math.max(1, Math.ceil((size.x / largest) * resolution));
  const ny = Math.max(1, Math.ceil((size.y / largest) * resolution));
  const nz = Math.max(1, Math.ceil((size.z / largest) * resolution));

  const step = new THREE.Vector3(size.x / nx, size.y / ny, size.z / nz);
  const cells = new Uint8Array(nx * ny * nz);

  const leftFlat = flattenedMesh(left);
  const rightFlat = flattenedMesh(right);

  const sample = new THREE.Vector3();
  let offset = 0;
  for (let z = 0; z < nz; z++) {
    for (let y = 0; y < ny; y++) {
      for (let x = 0; x < nx; x++) {
        sample.set(
          min.x + (x + 0.5) * step.x,
          min.y + (y + 0.5) * step.y,
          min.z + (z + 0.5) * step.z
        );

        const inLeft = isPointInsideMesh(sample, leftFlat);
        const inRight = isPointInsideMesh(sample, rightFlat);
        cells[offset++] = resultOccupancy(options.operation, inLeft, inRight) ? 1 : 0;
      }
    }
  }

  return { min, max, step, nx, ny, nz, cells };
}

function getCell(field: BooleanField, x: number, y: number, z: number): boolean {
  if (x < 0 || y < 0 || z < 0 || x >= field.nx || y >= field.ny || z >= field.nz) {
    return false;
  }

  const index = x + y * field.nx + z * field.nx * field.ny;
  return field.cells[index] === 1;
}

function pushQuad(
  positions: number[],
  normals: number[],
  indices: number[],
  v0: THREE.Vector3,
  v1: THREE.Vector3,
  v2: THREE.Vector3,
  v3: THREE.Vector3,
  normal: THREE.Vector3
) {
  const base = positions.length / 3;
  positions.push(v0.x, v0.y, v0.z, v1.x, v1.y, v1.z, v2.x, v2.y, v2.z, v3.x, v3.y, v3.z);

  for (let i = 0; i < 4; i++) {
    normals.push(normal.x, normal.y, normal.z);
  }

  indices.push(base, base + 1, base + 2, base, base + 2, base + 3);
}

function buildGeometryFromField(field: BooleanField): THREE.BufferGeometry {
  const positions: number[] = [];
  const normals: number[] = [];
  const indices: number[] = [];

  const origin = field.min;
  const step = field.step;

  for (let z = 0; z < field.nz; z++) {
    for (let y = 0; y < field.ny; y++) {
      for (let x = 0; x < field.nx; x++) {
        if (!getCell(field, x, y, z)) {
          continue;
        }

        const x0 = origin.x + x * step.x;
        const x1 = x0 + step.x;
        const y0 = origin.y + y * step.y;
        const y1 = y0 + step.y;
        const z0 = origin.z + z * step.z;
        const z1 = z0 + step.z;

        if (!getCell(field, x + 1, y, z)) {
          pushQuad(positions, normals, indices, new THREE.Vector3(x1, y0, z0), new THREE.Vector3(x1, y1, z0), new THREE.Vector3(x1, y1, z1), new THREE.Vector3(x1, y0, z1), new THREE.Vector3(1, 0, 0));
        }
        if (!getCell(field, x - 1, y, z)) {
          pushQuad(positions, normals, indices, new THREE.Vector3(x0, y0, z1), new THREE.Vector3(x0, y1, z1), new THREE.Vector3(x0, y1, z0), new THREE.Vector3(x0, y0, z0), new THREE.Vector3(-1, 0, 0));
        }
        if (!getCell(field, x, y + 1, z)) {
          pushQuad(positions, normals, indices, new THREE.Vector3(x0, y1, z1), new THREE.Vector3(x1, y1, z1), new THREE.Vector3(x1, y1, z0), new THREE.Vector3(x0, y1, z0), new THREE.Vector3(0, 1, 0));
        }
        if (!getCell(field, x, y - 1, z)) {
          pushQuad(positions, normals, indices, new THREE.Vector3(x0, y0, z0), new THREE.Vector3(x1, y0, z0), new THREE.Vector3(x1, y0, z1), new THREE.Vector3(x0, y0, z1), new THREE.Vector3(0, -1, 0));
        }
        if (!getCell(field, x, y, z + 1)) {
          pushQuad(positions, normals, indices, new THREE.Vector3(x1, y0, z1), new THREE.Vector3(x1, y1, z1), new THREE.Vector3(x0, y1, z1), new THREE.Vector3(x0, y0, z1), new THREE.Vector3(0, 0, 1));
        }
        if (!getCell(field, x, y, z - 1)) {
          pushQuad(positions, normals, indices, new THREE.Vector3(x0, y0, z0), new THREE.Vector3(x0, y1, z0), new THREE.Vector3(x1, y1, z0), new THREE.Vector3(x1, y0, z0), new THREE.Vector3(0, 0, -1));
        }
      }
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  geometry.setIndex(indices);
  geometry.computeBoundingBox();
  return geometry;
}

export function booleanOperation(
  left: THREE.Mesh,
  right: THREE.Mesh,
  options: BooleanOperationOptions
): BooleanShape {
  const field = createBooleanField(left, right, options);
  const geometry = buildGeometryFromField(field);

  if (options.constrainResultToPositiveY && geometry.boundingBox) {
    const minY = geometry.boundingBox.min.y;
    if (minY < 0) {
      geometry.translate(0, -minY, 0);
      geometry.computeBoundingBox();
    }
  }

  const booleanShape = new BooleanShape(
    left.uuid,
    right.uuid,
    options.operation,
    geometry,
    buildMaterial(options)
  );

  booleanShape.castShadow = true;
  booleanShape.receiveShadow = true;
  return booleanShape;
}
