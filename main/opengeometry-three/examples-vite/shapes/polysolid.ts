import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { numberControl, onNumberInput } from '../shared/controls';
import { requiredElement } from '../shared/dom';
import { startFpsMeter } from '../shared/fps';
import { bootExample } from '../shared/kernel';
import { releaseOnUnload } from '../shared/unload';
import { fitRenderer } from '../shared/viewport';
import {
  OpenGeometry, Solid, OG_PRIMITIVE_POLYSOLID, OG_PRIMITIVE_PARAMS_POLYSOLID,
} from '../../../dist/index.js';
import type { PolysolidParams } from '../../../dist/index.js';

type Justify = NonNullable<PolysolidParams['justify']>;

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
await bootExample('../');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
SCENE.add(new THREE.GridHelper(20, 20, 0x4460ff, 0xd5ddff));
const CAMERA = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
CAMERA.position.set(5.8, 4.4, 6.8);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(1.5, 0.5, -1);
CONTROLS.update();

type PathName = 'legs' | 'arc';

const CONFIG: { width: number; height: number; justify: Justify; path: PathName; outline: boolean } = {
  width: 0.2, height: 1, justify: 'centre', path: 'legs', outline: true,
};
const PATHS: Record<PathName, PolysolidParams['path']> = {
  legs: { start: [0, 0], segments: [{ to: [3, 0] }, { to: [3, 2] }], closed: false },
  arc: {
    start: [0, 0],
    segments: [{ to: [2, 0] }, { to: [3, 1], arc: { center: [2, 1], counterClockwise: true } }, { to: [3, 2] }],
    closed: false,
  },
};

function params(): PolysolidParams {
  return OG_PRIMITIVE_PARAMS_POLYSOLID({
    path: PATHS[CONFIG.path], width: CONFIG.width, height: CONFIG.height, justify: CONFIG.justify,
  });
}

const WALL = new Solid(OG_PRIMITIVE_POLYSOLID, params(), { ogId: 'polysolid' });
WALL.setAppearance({ color: 0x0ea5e9, outline: true, deflection: 0.005 });
SCENE.add(WALL);

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

let updateFrame = 0;
function updateWall(): void {
  const started = performance.now();
  try {
    WALL.rebuild(OG_PRIMITIVE_POLYSOLID, params());
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if (WALL.record?.revision !== WALL.lastInfo.shapeRevision) {
      throw new Error('Polysolid geometry did not finish');
    }
    STATUS.textContent = `Polysolid ready · ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) { STATUS.textContent = `Polysolid error: ${String(error)}`; }
}

function scheduleUpdate(): void {
  if (updateFrame) return;
  updateFrame = window.requestAnimationFrame(() => { updateFrame = 0; updateWall(); });
}

function attachNumberControl(name: 'width' | 'height'): void {
  onNumberInput(numberControl(name), (value) => {
    CONFIG[name] = value;
    scheduleUpdate();
  });
}

attachNumberControl('width');
attachNumberControl('height');
const JUSTIFY_OPTIONS: Justify[] = ['centre', 'left', 'right'];
const JUSTIFY_ROW = requiredElement('[data-control="justify"]', HTMLElement);
const JUSTIFY = requiredElement('[data-select]', HTMLSelectElement, JUSTIFY_ROW);
JUSTIFY.addEventListener('change', () => {
  CONFIG.justify = JUSTIFY_OPTIONS.find((option) => option === JUSTIFY.value) ?? 'centre';
  scheduleUpdate();
});
const PATH_OPTIONS: PathName[] = ['legs', 'arc'];
const PATH_ROW = requiredElement('[data-control="path"]', HTMLElement);
const PATH_SELECT = requiredElement('[data-select]', HTMLSelectElement, PATH_ROW);
PATH_SELECT.addEventListener('change', () => {
  CONFIG.path = PATH_OPTIONS.find((option) => option === PATH_SELECT.value) ?? 'legs';
  scheduleUpdate();
});
const OUTLINE_ROW = requiredElement('[data-control="outline"]', HTMLElement);
const OUTLINE = requiredElement('[data-toggle]', HTMLInputElement, OUTLINE_ROW);
const OUTLINE_STATE = requiredElement('[data-state]', HTMLElement, OUTLINE_ROW);
OUTLINE.addEventListener('change', () => {
  CONFIG.outline = OUTLINE.checked;
  OUTLINE_STATE.textContent = OUTLINE.checked ? 'Enabled' : 'Disabled';
  WALL.setAppearance({ outline: OUTLINE.checked });
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
if (READY.failed.length) throw new Error('Polysolid geometry failed');
render();
STATUS.textContent = 'Polysolid ready';
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  WALL.dispose();
});
