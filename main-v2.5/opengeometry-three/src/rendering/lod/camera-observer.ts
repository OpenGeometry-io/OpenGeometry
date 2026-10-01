import * as THREE from 'three';
import { runtime } from '../../runtime/runtime-state';
import { observe } from './lod-controller';

export function observeCamera(renderer: THREE.WebGLRenderer, camera: THREE.Camera): void {
  observe(runtime(), camera, renderer.getDrawingBufferSize(new THREE.Vector2()).y, Date.now());
}
