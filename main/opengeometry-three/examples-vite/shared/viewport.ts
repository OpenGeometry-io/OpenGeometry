import type * as THREE from 'three';

export type ExampleView = { resetView(): void; render(): void };

export function fitRenderer(
  container: HTMLElement,
  camera: THREE.PerspectiveCamera,
  renderer: THREE.WebGLRenderer,
): number {
  const width = Math.max(1, container.clientWidth);
  const height = Math.max(1, container.clientHeight);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  renderer.setSize(width, height, false);
  return width;
}

export function narrowAwareResize(
  container: HTMLElement,
  camera: THREE.PerspectiveCamera,
  renderer: THREE.WebGLRenderer,
  view: ExampleView,
): () => void {
  let previousNarrow: boolean | undefined;
  return () => {
    const narrow = fitRenderer(container, camera, renderer) < 700;
    if (previousNarrow !== narrow) { previousNarrow = narrow; view.resetView(); }
    else view.render();
  };
}
