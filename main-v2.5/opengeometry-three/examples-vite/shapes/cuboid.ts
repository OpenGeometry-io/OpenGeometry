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
await bootExample('../');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.GridHelper(20, 20, 0x4460ff, 0xd5ddff));
const CAMERA = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
CAMERA.position.set(5.8, 4.4, 6.8);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(0, 0.8, 0);
CONTROLS.update();

const CONFIG = { width: 2, height: 1.6, depth: 1.2, outline: true };
const CUBOID = new Solid(
  OG_PRIMITIVE_CUBOID, { width: CONFIG.width, height: CONFIG.height, depth: CONFIG.depth }, { ogId: 'cuboid' },
);
CUBOID.setAppearance({ color: 0x0ea5e9, outline: true, deflection: 0.005 });
SCENE.add(CUBOID);

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

let updateFrame = 0;
function updateCuboid(): void {
  const started = performance.now();
  try {
    CUBOID.rebuild(OG_PRIMITIVE_CUBOID, { width: CONFIG.width, height: CONFIG.height, depth: CONFIG.depth });
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if (CUBOID.record?.revision !== CUBOID.lastInfo.shapeRevision) {
      throw new Error('Cuboid geometry did not finish');
    }
    STATUS.textContent = `Cuboid ready · ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) { STATUS.textContent = `Cuboid error: ${String(error)}`; }
}

function scheduleUpdate(): void {
  if (updateFrame) return;
  updateFrame = window.requestAnimationFrame(() => { updateFrame = 0; updateCuboid(); });
}

function attachNumberControl(name: 'width' | 'height' | 'depth'): void {
  onNumberInput(numberControl(name), (value) => {
    CONFIG[name] = value;
    scheduleUpdate();
  });
}

attachNumberControl('width');
attachNumberControl('height');
attachNumberControl('depth');
const OUTLINE_ROW = requiredElement('[data-control="outline"]', HTMLElement);
const OUTLINE = requiredElement('[data-toggle]', HTMLInputElement, OUTLINE_ROW);
const OUTLINE_STATE = requiredElement('[data-state]', HTMLElement, OUTLINE_ROW);
OUTLINE.addEventListener('change', () => {
  CONFIG.outline = OUTLINE.checked;
  OUTLINE_STATE.textContent = OUTLINE.checked ? 'Enabled' : 'Disabled';
  CUBOID.setAppearance({ outline: OUTLINE.checked });
  render();
});

const RESIZE_OBSERVER = new ResizeObserver(resize);
RESIZE_OBSERVER.observe(APP);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => {
  STATUS.textContent = `Geometry error: ${String(error)}`;
});
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
