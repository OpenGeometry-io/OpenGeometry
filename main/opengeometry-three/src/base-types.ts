import * as THREE from 'three';

export const OPEN_GEOMETRY_THREE_VERSION = '0.0.1';

export interface OpenGeometryOptions {
  container: HTMLElement;
  scene: THREE.Scene;
  camera: THREE.Camera;
  wasmURL: string;
}