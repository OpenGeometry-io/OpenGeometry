import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { numberControl, onNumberInput } from '../shared/controls.js';
import { requiredElement } from '../shared/dom.js';
import { startFpsMeter } from '../shared/fps.js';
import { bootExample } from '../shared/kernel.js';
import { releaseOnUnload } from '../shared/unload.js';
import { narrowAwareResize } from '../shared/viewport.js';
import {
  OpenGeometry, Solid,
  OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';

const VIEWPORT = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const WIDTH_INPUT = requiredElement('[data-control="width"] [data-range]', HTMLInputElement);
const HEIGHT_INPUT = requiredElement('[data-control="height"] [data-range]', HTMLInputElement);
const DEPTH_INPUT = requiredElement('[data-control="depth"] [data-range]', HTMLInputElement);
const CENTER_INPUT = requiredElement('[data-control="center-y"] [data-range]', HTMLInputElement);
await bootExample('./');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.GridHelper(20, 20, 0x4460ff, 0xd5ddff));
const CAMERA = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
VIEWPORT.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.enableDamping = false;
CONTROLS.minDistance = 3;
CONTROLS.maxDistance = 30;

type OpeningBodies = { wall: Solid; cutter: Solid };
type OpeningValues = { width: number; height: number; depth: number; centerY: number };

let revision = 0;
let current: OpeningBodies | undefined;
let updateFrame = 0;
const VALUES = (): OpeningValues => ({
  width: Number(WIDTH_INPUT.value), height: Number(HEIGHT_INPUT.value),
  depth: Number(DEPTH_INPUT.value), centerY: Number(CENTER_INPUT.value),
});

function createExample(): OpeningBodies {
  const id = ++revision;
  const created: Solid[] = [];
  try {
    const wallSize = { width: 4.4, height: 3.2, depth: 0.32 };
    const wall = new Solid(OG_PRIMITIVE_CUBOID, wallSize, { ogId: `wall-${String(id)}` });
    created.push(wall);
    const { width, height, depth, centerY } = VALUES();
    const cutter = new Solid(OG_PRIMITIVE_CUBOID, { width, height, depth }, { ogId: `opening-${String(id)}` });
    created.push(cutter);
    cutter.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, centerY - height / 2, 0] });
    wall.operate(OG_OPERATION_SUBTRACT, { tools: [cutter] });
    wall.setAppearance({ color: 0xcbd5e1, outline: true, deflection: 0.005 });
    cutter.setAppearance({ color: 0xf97316, opacity: 0.22, outline: true, deflection: 0.005 });
    return { wall, cutter };
  } catch (error) {
    created.slice().reverse().forEach((body) => { body.dispose(); });
    throw error;
  }
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resetView(): void {
  const narrow = VIEWPORT.clientWidth < 700;
  CAMERA.position.set(narrow ? 6 : 6, 4.5, narrow ? 12 : 8);
  CONTROLS.target.set(0, 1.55, 0);
  CONTROLS.update();
  render();
}

const RESIZE = narrowAwareResize(VIEWPORT, CAMERA, RENDERER, { resetView, render });

function finished(body: Solid): boolean {
  return body.record?.revision === body.lastInfo.shapeRevision;
}

function rebuild(): void {
  let next: OpeningBodies | undefined;
  const started = performance.now();
  try {
    next = createExample();
    SCENE.add(next.wall, next.cutter);
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if (!finished(next.wall) || !finished(next.cutter)) throw new Error('Opening geometry did not finish');
    const previous = current;
    current = next;
    if (previous) { previous.cutter.dispose(); previous.wall.dispose(); }
    const faces = String(next.wall.getBrep().topology.faces.length);
    STATUS.textContent = `Opening ready · ${faces} wall faces · ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) {
    if (next && next !== current) { next.cutter.dispose(); next.wall.dispose(); }
    STATUS.textContent = `Opening error: ${String(error)}`;
  }
}

function scheduleRebuild(): void {
  if (updateFrame) return;
  updateFrame = window.requestAnimationFrame(() => { updateFrame = 0; rebuild(); });
}

function rebuildNow(): void {
  if (updateFrame) window.cancelAnimationFrame(updateFrame);
  updateFrame = 0;
  rebuild();
}

function attachNumberControl(name: string): void {
  const control = numberControl(name);
  onNumberInput(control, () => { scheduleRebuild(); });
  control.range.addEventListener('change', rebuildNow);
  control.number.addEventListener('change', rebuildNow);
}

attachNumberControl('width');
attachNumberControl('height');
attachNumberControl('depth');
attachNumberControl('center-y');

const RESIZE_OBSERVER = new ResizeObserver(RESIZE);
RESIZE_OBSERVER.observe(VIEWPORT);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => {
  STATUS.textContent = `Geometry error: ${String(error)}`;
});
RESIZE();
resetView();
rebuild();
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

(window as typeof window & { __ogOpeningExample?: unknown }).__ogOpeningExample = {
  camera: CAMERA, controls: CONTROLS, renderer: RENDERER,
  get current() { return current; },
  render, rebuild,
};

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  if (current) { current.cutter.dispose(); current.wall.dispose(); }
});
