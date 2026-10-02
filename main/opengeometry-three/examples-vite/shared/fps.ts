import type * as THREE from 'three';
import type { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import Stats from 'three/examples/jsm/libs/stats.module.js';

export function startFpsMeter(
  renderer: THREE.WebGLRenderer,
  scene: THREE.Scene,
  camera: THREE.Camera,
  controls: OrbitControls,
): () => void {
  const stats = new Stats();
  stats.showPanel(0);
  stats.dom.style.position = 'fixed';
  stats.dom.style.left = '12px';
  stats.dom.style.bottom = '12px';
  stats.dom.style.top = 'auto';
  stats.dom.style.zIndex = '1000';
  document.body.append(stats.dom);
  renderer.setAnimationLoop(() => {
    if (document.hidden) return;
    controls.update();
    renderer.render(scene, camera);
    stats.update();
  });
  return () => {
    renderer.setAnimationLoop(null);
    stats.dom.remove();
  };
}
