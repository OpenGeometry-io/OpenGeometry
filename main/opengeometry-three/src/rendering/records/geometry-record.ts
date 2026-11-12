import * as THREE from 'three';
import type { DisplayBuffers } from '../../dto/display-buffers';
import { validateDisplayBuffers } from './validate-display-buffers';

export function recordKey(shapeId: string, revision: number, bucket: number, smoothAngle?: number): string {
  return `${shapeId}@${String(revision)}#${String(Math.log2(bucket))}~${String(smoothAngle)}`;
}

export class GeometryRecord {
  readonly key: string;
  readonly surface: THREE.BufferGeometry | null;
  readonly outline: THREE.BufferGeometry;
  readonly origin: THREE.Vector3;
  readonly faceRanges: Uint32Array;
  readonly edgeIds: Uint32Array;
  readonly smoothEdges: Uint8Array;
  readonly revision: number;
  readonly bucket: number;
  readonly triangles: number;
  readonly bytes: number;
  refs = 0;
  lastUse = 0;

  constructor(readonly shapeId: string, buffers: DisplayBuffers, smoothAngle?: number) {
    validateDisplayBuffers(buffers);
    this.key = recordKey(shapeId, buffers.revision, buffers.bucket, smoothAngle);
    this.revision = buffers.revision;
    this.bucket = buffers.bucket;
    this.triangles = buffers.triangles;
    this.origin = new THREE.Vector3(buffers.origin[0], buffers.origin[1], buffers.origin[2]);
    this.faceRanges = buffers.faceRanges;
    this.edgeIds = buffers.edgeIds;
    this.smoothEdges = buffers.smoothEdges;
    this.bytes = buffers.positions.byteLength + buffers.normals.byteLength + buffers.indices.byteLength
      + buffers.faceRanges.byteLength + buffers.outline.byteLength + buffers.edgeIds.byteLength
      + buffers.smoothEdges.byteLength;
    this.surface = buffers.indices.length ? new THREE.BufferGeometry() : null;
    if (this.surface) {
      this.surface.setAttribute('position', new THREE.BufferAttribute(buffers.positions, 3));
      this.surface.setAttribute('normal', new THREE.BufferAttribute(buffers.normals, 3));
      this.surface.setIndex(new THREE.BufferAttribute(buffers.indices, 1));
      this.surface.computeBoundingBox();
      this.surface.computeBoundingSphere();
    }
    this.outline = new THREE.BufferGeometry();
    this.outline.setAttribute('position', new THREE.BufferAttribute(buffers.outline, 3));
    if (buffers.outline.length) {
      this.outline.computeBoundingBox();
      this.outline.computeBoundingSphere();
    }
  }

  dispose(): void {
    this.surface?.dispose();
    this.outline.dispose();
  }
}
