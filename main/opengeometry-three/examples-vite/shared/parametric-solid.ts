import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { numberControl, onNumberInput } from './controls';
import { requiredElement } from './dom';
import { startFpsMeter } from './fps';
import { bootExample } from './kernel';
import { releaseOnUnload } from './unload';
import { fitRenderer } from './viewport';
import { OpenGeometry, Solid } from '../../../dist/index.js';

export type ParametricSolid = { kind: string; title: string; params: Record<string, number> };

type Stage = { scene: THREE.Scene; camera: THREE.PerspectiveCamera; renderer: THREE.WebGLRenderer };

function createStage(app: HTMLElement): Stage {
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0xeef2ff);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
  const light = new THREE.DirectionalLight(0xffffff, 2);
  light.position.set(5, 8, 5);
  scene.add(light);
  scene.add(new THREE.GridHelper(10, 20, 0x4460ff, 0xd5ddff));
  const camera = new THREE.PerspectiveCamera(50, 1, 0.05, 100);
  camera.position.set(2.2, 1.8, 2.6);
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
  app.append(renderer.domElement);
  return { scene, camera, renderer };
}

function rebuildOnInput(page: ParametricSolid, solid: Solid, scene: THREE.Scene, status: HTMLElement): () => void {
  let frame = 0;
  const update = (): void => {
    const started = performance.now();
    try {
      solid.rebuild(page.kind, page.params);
      scene.updateMatrixWorld(true);
      OpenGeometry.flush({ geometry: 'sync' });
      if (solid.record?.revision !== solid.lastInfo.shapeRevision) {
        throw new Error(`${page.title} geometry did not finish`);
      }
      status.textContent = `${page.title} ready · ${(performance.now() - started).toFixed(1)} ms`;
    } catch (error) { status.textContent = `${page.title} error: ${String(error)}`; }
  };
  for (const name of Object.keys(page.params)) {
    onNumberInput(numberControl(name), (value) => {
      page.params[name] = value;
      if (frame) return;
      frame = window.requestAnimationFrame(() => { frame = 0; update(); });
    });
  }
  return () => { if (frame) window.cancelAnimationFrame(frame); };
}

export async function startParametricSolid(page: ParametricSolid): Promise<void> {
  const app = requiredElement('#app', HTMLDivElement);
  const status = requiredElement('#status', HTMLElement);
  await bootExample('../');
  const { scene, camera, renderer } = createStage(app);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(0, 0.3, 0);
  controls.update();
  const solid = new Solid(page.kind, page.params, { ogId: page.kind });
  solid.setAppearance({ color: 0x0ea5e9, outline: true, deflection: 0.002 });
  scene.add(solid);
  const render = (): void => { renderer.render(scene, camera); };
  const resizeObserver = new ResizeObserver(() => { fitRenderer(app, camera, renderer); render(); });
  resizeObserver.observe(app);
  const cancelUpdate = rebuildOnInput(page, solid, scene, status);
  controls.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
  controls.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
  const unsubscribeError = OpenGeometry.on('error', (error) => {
    status.textContent = `Geometry error: ${String(error)}`;
  });
  const ready = await OpenGeometry.settled();
  if (ready.failed.length) throw new Error(`${page.title} geometry failed`);
  render();
  status.textContent = `${page.title} ready`;
  const stopFpsMeter = startFpsMeter(renderer, scene, camera, controls);
  releaseOnUnload(
    { resizeObserver, stopFpsMeter, unsubscribeError, controls, renderer }, cancelUpdate, () => { solid.dispose(); },
  );
}
