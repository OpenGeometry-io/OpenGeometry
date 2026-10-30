import type * as THREE from 'three';
import type { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { OpenGeometry } from '../../../dist/index.js';

export type ExampleResources = {
  resizeObserver: ResizeObserver;
  stopFpsMeter: () => void;
  unsubscribeError: () => void;
  controls: OrbitControls;
  renderer: THREE.WebGLRenderer;
};

export function releaseOnUnload(
  resources: ExampleResources,
  cancelUpdate: () => void,
  disposeBodies: () => void,
): void {
  window.addEventListener('beforeunload', () => {
    cancelUpdate();
    resources.resizeObserver.disconnect();
    resources.stopFpsMeter();
    resources.unsubscribeError();
    resources.controls.dispose();
    disposeBodies();
    OpenGeometry.reset();
    resources.renderer.dispose();
    resources.renderer.forceContextLoss();
  }, { once: true });
}
