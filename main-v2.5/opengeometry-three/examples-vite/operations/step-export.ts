import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { numberControl, onNumberInput } from '../shared/controls.js';
import { requiredElement } from '../shared/dom.js';
import { startFpsMeter } from '../shared/fps.js';
import { bootExample } from '../shared/kernel.js';
import { releaseOnUnload } from '../shared/unload.js';
import { fitRenderer } from '../shared/viewport.js';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID } from '../../../dist/index.js';

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const REPORT = requiredElement('#export-report', HTMLElement);
const ERROR = requiredElement('#error', HTMLElement);
const DOWNLOAD = requiredElement('[data-download]', HTMLAnchorElement);
const EXPORT_BUTTON = requiredElement('[data-export]', HTMLButtonElement);
await bootExample('../');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.GridHelper(20, 20, 0x4460ff, 0xd5ddff));
const CAMERA = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
CAMERA.position.set(5.8, 4.5, 6.8);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(0, 0.9, 0);
CONTROLS.update();

const DIMENSIONS = { width: 1.8, height: 1.4, depth: 1.1 };
const CUBOID = new Solid(OG_PRIMITIVE_CUBOID, DIMENSIONS, { ogId: 'step-cuboid' });
CUBOID.setAppearance({ color: 0x0ea5e9, outline: true, deflection: 0.005 });
SCENE.add(CUBOID);

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

let updateFrame = 0;
function updateCuboid(): boolean {
  const started = performance.now();
  try {
    CUBOID.rebuild(OG_PRIMITIVE_CUBOID, DIMENSIONS);
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if (CUBOID.record?.revision !== CUBOID.lastInfo.shapeRevision) {
      throw new Error('Cuboid geometry did not finish');
    }
    STATUS.textContent = `Cuboid ready · ${(performance.now() - started).toFixed(1)} ms`;
    ERROR.textContent = '';
    return true;
  } catch (failure) { ERROR.textContent = String(failure); return false; }
}

function scheduleUpdate(): void {
  if (updateFrame) return;
  updateFrame = window.requestAnimationFrame(() => { updateFrame = 0; updateCuboid(); });
}

for (const name of ['width', 'height', 'depth'] as const) {
  onNumberInput(numberControl(name), (value) => {
    DIMENSIONS[name] = value;
    scheduleUpdate();
  });
}

async function exportCuboid(): Promise<void> {
  try {
    EXPORT_BUTTON.disabled = true;
    if (updateFrame) { window.cancelAnimationFrame(updateFrame); updateFrame = 0; if (!updateCuboid()) return; }
    const result = await OpenGeometry.exportStep({ nodes: [CUBOID], unit: 'metre', name: 'OpenGeometry cuboid' });
    REPORT.textContent = JSON.stringify(result.report, null, 2);
    ERROR.textContent = '';
    const url = URL.createObjectURL(new Blob([result.text], { type: 'application/step' }));
    DOWNLOAD.href = url;
    DOWNLOAD.download = 'opengeometry-cuboid.step';
    DOWNLOAD.click();
    window.setTimeout(() => { URL.revokeObjectURL(url); }, 1000);
  } catch (failure) { ERROR.textContent = `Export failed: ${String(failure)}`; }
  finally { EXPORT_BUTTON.disabled = false; }
}
EXPORT_BUTTON.addEventListener('click', () => { void exportCuboid(); });

const RESIZE_OBSERVER = new ResizeObserver(resize);
RESIZE_OBSERVER.observe(APP);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (failure) => { ERROR.textContent = String(failure); });
resize();
const READY = await OpenGeometry.settled();
if (READY.failed.length) throw new Error('Cuboid geometry failed');
render();
STATUS.textContent = 'Cuboid ready';
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  CUBOID.dispose();
});
