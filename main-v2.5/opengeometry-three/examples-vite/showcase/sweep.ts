import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { numberControl, onNumberInput } from '../shared/controls.js';
import { requiredElement } from '../shared/dom.js';
import { startFpsMeter } from '../shared/fps.js';
import { bootExample } from '../shared/kernel.js';
import { releaseOnUnload } from '../shared/unload.js';
import { fitRenderer } from '../shared/viewport.js';
import {
  OpenGeometry, Solid, Wire,
  OG_OPERATION_SWEEP, OG_PRIMITIVE_POLYLINE,
} from '../../../dist/index.js';

type Point = [number, number, number];
type Path = [Point, ...Point[]];
type Frame = {
  profile: Wire;
  path: Wire;
  solid: Solid;
  guide: THREE.Line<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  profileGuide: THREE.LineLoop<THREE.BufferGeometry, THREE.LineBasicMaterial>;
};

const VIEWPORT = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
await bootExample('./');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
const CAMERA = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
VIEWPORT.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.enableDamping = false;
CONTROLS.minDistance = 4;
CONTROLS.maxDistance = 40;

const CONFIG = { topRight: 2.3, topLeft: -1.25, width: 0.24, depth: 0.22 };
const FRAMES: Frame[] = [];
const GUIDE_MATERIAL = (color: number): THREE.LineBasicMaterial =>
  new THREE.LineBasicMaterial({ color, depthTest: false });
const PROFILE_POINTS = (start: Point, width: number, depth: number): Point[] => [
  [start[0], start[1] - width / 2, start[2] - depth / 2],
  [start[0], start[1] + width / 2, start[2] - depth / 2],
  [start[0], start[1] + width / 2, start[2] + depth / 2],
  [start[0], start[1] - width / 2, start[2] + depth / 2],
];
const PRIMARY_PATH = (): Path => [
  [-1.6, 0.1, 0], [1.6, 0.1, 0], [1.15, CONFIG.topRight, 0], [CONFIG.topLeft, 1.95, 0],
];

function setGuide(guide: THREE.Line, points: Point[]): void {
  guide.geometry.dispose();
  guide.geometry = new THREE.BufferGeometry().setFromPoints(points.map((point) => new THREE.Vector3(...point)));
}

function addFrame(id: string, points: Path, width: number, depth: number, color: number, guideColor: number): Frame {
  const path = new Wire(OG_PRIMITIVE_POLYLINE, { points, closed: false }, { ogId: `${id}-path` });
  const profile = new Wire(
    OG_PRIMITIVE_POLYLINE, { points: PROFILE_POINTS(points[0], width, depth), closed: true }, { ogId: `${id}-profile` },
  );
  const solid = new Solid(OG_OPERATION_SWEEP, { profile, path }, { ogId: id });
  solid.setAppearance({ color, outline: true, deflection: 0.01 });
  const guide = new THREE.Line(new THREE.BufferGeometry(), GUIDE_MATERIAL(guideColor));
  const profileGuide = new THREE.LineLoop(new THREE.BufferGeometry(), GUIDE_MATERIAL(0xc54b42));
  guide.position.z = 0.18;
  guide.renderOrder = 10;
  profileGuide.position.z = 0.24;
  profileGuide.renderOrder = 11;
  setGuide(guide, points);
  setGuide(profileGuide, PROFILE_POINTS(points[0], width, depth));
  SCENE.add(solid, guide, profileGuide);
  const frame = { profile, path, solid, guide, profileGuide };
  FRAMES.push(frame);
  return frame;
}

const PRIMARY = addFrame('adjustable-frame', PRIMARY_PATH(), CONFIG.width, CONFIG.depth, 0x3b82f6, 0x111827);
const DOOR = addFrame(
  'door-frame', [[-6.1, 0.1, 0], [-4.3, 0.1, 0], [-4.3, 2.8, 0], [-6.1, 2.8, 0]], 0.2, 0.18, 0xd97706, 0x7c2d12,
);
const WINDOW_FRAME = addFrame(
  'window-frame', [[3.8, 0.75, 0], [6.4, 0.75, 0], [6.1, 2.35, 0], [4.1, 2.1, 0]], 0.18, 0.16, 0x0f766e, 0x134e4a,
);
SCENE.add(new THREE.GridHelper(20, 20, 0x4460ff, 0xd5ddff));

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resetView(): void {
  const narrow = VIEWPORT.clientWidth < 700;
  DOOR.solid.visible = DOOR.guide.visible = DOOR.profileGuide.visible = !narrow;
  WINDOW_FRAME.solid.visible = WINDOW_FRAME.guide.visible = WINDOW_FRAME.profileGuide.visible = !narrow;
  CAMERA.position.set(narrow ? 1 : 0.6, 2.7, narrow ? 15 : 17.5);
  CONTROLS.target.set(narrow ? 0.6 : 0, 1.35, 0);
  CONTROLS.update();
  render();
}

let previousNarrow: boolean | undefined;
function resize(): void {
  const narrow = fitRenderer(VIEWPORT, CAMERA, RENDERER) < 700;
  DOOR.solid.visible = DOOR.guide.visible = DOOR.profileGuide.visible = !narrow;
  WINDOW_FRAME.solid.visible = WINDOW_FRAME.guide.visible = WINDOW_FRAME.profileGuide.visible = !narrow;
  if (previousNarrow !== narrow) {
    previousNarrow = narrow;
    resetView();
  } else render();
}

let updateFrame = 0;
function updateSweep(): void {
  const points = PRIMARY_PATH();
  const started = performance.now();
  try {
    PRIMARY.path.rebuild(OG_PRIMITIVE_POLYLINE, { points, closed: false });
    PRIMARY.profile.rebuild(OG_PRIMITIVE_POLYLINE, {
      points: PROFILE_POINTS(points[0], CONFIG.width, CONFIG.depth), closed: true,
    });
    PRIMARY.solid.rebuild(OG_OPERATION_SWEEP, { profile: PRIMARY.profile, path: PRIMARY.path });
    setGuide(PRIMARY.guide, points);
    setGuide(PRIMARY.profileGuide, PROFILE_POINTS(points[0], CONFIG.width, CONFIG.depth));
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if (PRIMARY.solid.record?.revision !== PRIMARY.solid.lastInfo.shapeRevision) {
      throw new Error('Sweep geometry did not finish');
    }
    STATUS.textContent = `Sweep ready · ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) {
    STATUS.textContent = `Sweep error: ${String(error)}`;
  }
}

function scheduleUpdate(): void {
  if (updateFrame) return;
  updateFrame = window.requestAnimationFrame(() => { updateFrame = 0; updateSweep(); });
}

function updateNow(): void {
  if (updateFrame) window.cancelAnimationFrame(updateFrame);
  updateFrame = 0;
  updateSweep();
}

function attachNumberControl(name: string, key: keyof typeof CONFIG): void {
  const control = numberControl(name);
  onNumberInput(control, (value) => {
    CONFIG[key] = value;
    scheduleUpdate();
  });
  control.range.addEventListener('change', updateNow);
  control.number.addEventListener('change', updateNow);
}

attachNumberControl('top-right-y', 'topRight');
attachNumberControl('top-left-x', 'topLeft');
attachNumberControl('frame-width', 'width');
attachNumberControl('frame-depth', 'depth');

const RESIZE_OBSERVER = new ResizeObserver(resize);
RESIZE_OBSERVER.observe(VIEWPORT);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => {
  STATUS.textContent = `Geometry error: ${String(error)}`;
});

resize();
resetView();
const READY = await OpenGeometry.settled();
if (READY.failed.length) throw new Error(`${String(READY.failed.length)} sweeps failed to tessellate`);
render();
STATUS.textContent = 'Sweep ready';
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

(window as typeof window & { __ogSweepExample?: unknown }).__ogSweepExample = {
  camera: CAMERA, controls: CONTROLS, renderer: RENDERER, frames: FRAMES,
  render, resetView, updateSweep,
  exportStep: () => OpenGeometry.exportStep({ nodes: FRAMES.map((frame) => frame.solid) }),
};

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  FRAMES.slice().reverse().forEach((frame) => {
    frame.solid.dispose();
    frame.profile.dispose();
    frame.path.dispose();
    frame.guide.geometry.dispose();
    frame.guide.material.dispose();
    frame.profileGuide.geometry.dispose();
    frame.profileGuide.material.dispose();
  });
});
