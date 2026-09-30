import * as THREE from 'three';
import type { GeometryRecord } from '../rendering/records/geometry-record.js';
import { runtime } from '../runtime/runtime-state.js';

export class DisplayClone extends THREE.Group {
  constructor(
    readonly sourceRecord: GeometryRecord | undefined,
    readonly mesh: THREE.Mesh<THREE.BufferGeometry, THREE.Material>,
    readonly lines: THREE.LineSegments<THREE.BufferGeometry, THREE.Material>,
  ) {
    super();
    this.add(mesh, lines);
    if (sourceRecord) runtime().records.retain(sourceRecord);
  }

  dispose(): void {
    if (this.sourceRecord) runtime().records.release(this.sourceRecord);
    this.mesh.material.dispose();
    this.lines.material.dispose();
    this.parent?.remove(this);
  }
}
