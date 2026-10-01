import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { requiredElement } from '../shared/dom.js';
import { startFpsMeter } from '../shared/fps.js';
import { bootExample } from '../shared/kernel.js';
import { releaseOnUnload } from '../shared/unload.js';
import { fitRenderer } from '../shared/viewport.js';
import { OpenGeometry, Solid, OG_PRIMITIVE_CYLINDER } from '../../../dist/index.js';

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const RADIUS_INPUT = requiredElement('#radius', HTMLInputElement);
const HEIGHT_INPUT = requiredElement('#height', HTMLInputElement);
const DEFLECTION_INPUT = requiredElement('#deflection', HTMLInputElement);
const WIREFRAME_INPUT = requiredElement('#wireframe', HTMLInputElement);
const OUTLINE_INPUT = requiredElement('#outline', HTMLInputElement);
const STATISTICS = requiredElement('#statistics', HTMLElement);
const SELECTION = requiredElement('#selection', HTMLElement);
await bootExample('../');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
SCENE.add(new THREE.GridHelper(16, 16, 0x7188aa, 0xcbd5e1));
const CAMERA = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
CAMERA.position.set(6, 4.5, 6);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(0, 0.8, 0);
CONTROLS.update();

const CYLINDER = new Solid(OG_PRIMITIVE_CYLINDER, {
  radius: Number(RADIUS_INPUT.value), height: Number(HEIGHT_INPUT.value),
}, { ogId: 'cylinder' });
CYLINDER.setAppearance({ color: 0x329ac7, outline: true, deflection: 10 ** Number(DEFLECTION_INPUT.value) });
SCENE.add(CYLINDER);

function applyWireframe(): void {
  const material = CYLINDER.surface.material;
  if (Array.isArray(material) || !('wireframe' in material)) throw new Error('The cylinder surface has no wireframe');
  if (material.wireframe !== WIREFRAME_INPUT.checked) {
    material.wireframe = WIREFRAME_INPUT.checked;
    material.needsUpdate = true;
  }
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

function showStatistics(): void {
  const faces = String(CYLINDER.getBrep().topology.faces.length);
  const triangles = (CYLINDER.record?.triangles ?? 0).toLocaleString();
  STATISTICS.textContent = `${faces} analytic faces · ${triangles} render triangles`;
}

function updateCylinder(rebuild: boolean): void {
  const started = performance.now();
  try {
    if (rebuild) {
      CYLINDER.rebuild(OG_PRIMITIVE_CYLINDER, {
        radius: Number(RADIUS_INPUT.value), height: Number(HEIGHT_INPUT.value),
      });
    }
    CYLINDER.setAppearance({ outline: OUTLINE_INPUT.checked, deflection: 10 ** Number(DEFLECTION_INPUT.value) });
    applyWireframe();
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if (CYLINDER.record?.revision !== CYLINDER.lastInfo.shapeRevision) {
      throw new Error('Cylinder geometry did not finish');
    }
    showStatistics();
    STATUS.textContent = `Cylinder ready · ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) { STATUS.textContent = `Cylinder error: ${String(error)}`; }
}

let updateFrame = 0;
function scheduleUpdate(rebuild: boolean): void {
  if (updateFrame) window.cancelAnimationFrame(updateFrame);
  updateFrame = window.requestAnimationFrame(() => { updateFrame = 0; updateCylinder(rebuild); });
}

RADIUS_INPUT.addEventListener('change', () => { scheduleUpdate(true); });
HEIGHT_INPUT.addEventListener('change', () => { scheduleUpdate(true); });
DEFLECTION_INPUT.addEventListener('input', () => { scheduleUpdate(false); });
WIREFRAME_INPUT.addEventListener('change', () => { applyWireframe(); render(); });
OUTLINE_INPUT.addEventListener('change', () => {
  CYLINDER.setAppearance({ outline: OUTLINE_INPUT.checked });
  applyWireframe();
  render();
});
const RAYCASTER = new THREE.Raycaster();
RENDERER.domElement.addEventListener('pointerdown', (event) => {
  const bounds = RENDERER.domElement.getBoundingClientRect();
  const pointer = new THREE.Vector2(
    ((event.clientX - bounds.left) / bounds.width) * 2 - 1, -((event.clientY - bounds.top) / bounds.height) * 2 + 1,
  );
  RAYCASTER.setFromCamera(pointer, CAMERA);
  const hit = RAYCASTER.intersectObject(CYLINDER.surface)[0];
  if (!hit) return;
  const resolved = OpenGeometry.resolveHit(hit);
  if (resolved?.faceId !== undefined) SELECTION.textContent = `Face ${String(resolved.faceId)}`;
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
if (READY.failed.length) throw new Error('Cylinder geometry failed');
applyWireframe();
render();
showStatistics();
STATUS.textContent = 'Cylinder ready';
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  CYLINDER.dispose();
});
