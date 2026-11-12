import type * as THREE from 'three';
import { runtime } from '../../runtime/runtime-state';
import { observe } from './lod-controller';

export function observeCamera(renderer: THREE.WebGLRenderer, camera: THREE.Camera): void {
  const state = runtime();
  const { frames, bufferSize } = state.lod;
  const frame = renderer.info.render.frame;
  if (frames.get(renderer) === frame) return;
  frames.set(renderer, frame);
  observe(state, camera, renderer.getDrawingBufferSize(bufferSize).y, Date.now());
}
