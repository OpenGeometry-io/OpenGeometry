import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { requiredElement } from '../shared/dom.js';
import { startFpsMeter } from '../shared/fps.js';
import { bootExample } from '../shared/kernel.js';
import { releaseOnUnload } from '../shared/unload.js';
import { fitRenderer } from '../shared/viewport.js';
import {
  OpenGeometry, Solid, SystemAssembly, Wire,
  OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT, OG_OPERATION_SWEEP,
  OG_PRIMITIVE_CIRCLE, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_CYLINDER,
  OG_PRIMITIVE_POLYLINE, OG_PRIMITIVE_RECTANGLE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';

const VIEWPORT = requiredElement('#viewport', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const RESET_BUTTON = requiredElement('#reset-view', HTMLButtonElement);
const EDGES_BUTTON = requiredElement('#toggle-edges', HTMLButtonElement);
const EXPORT_BUTTON = requiredElement('#export-step', HTMLButtonElement);
await bootExample('./');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xf1f5f8);
const CAMERA = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
VIEWPORT.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.enableDamping = false;
CONTROLS.minDistance = 4;
CONTROLS.maxDistance = 35;
CONTROLS.maxPolarAngle = Math.PI * 0.93;

const SOLIDS: Solid[] = [];
const LEVEL = new SystemAssembly({ ogId: 'example-level' });
const PLACE = (solid: Solid, offset: [number, number, number], color: number): Solid => {
  solid.transform(OG_TRANSFORM_TRANSLATE, { offset });
  solid.setAppearance({ color });
  SOLIDS.push(solid);
  SCENE.add(solid);
  return solid;
};

PLACE(new Solid(OG_PRIMITIVE_CUBOID, { width: 7, height: 0.24, depth: 6 }, { ogId: 'floor' }), [0, -0.24, 0], 0xc7d6df);
const FLOOR_SURFACE = { width: 6.6, height: 0.07, depth: 5.6 };
PLACE(new Solid(OG_PRIMITIVE_CUBOID, FLOOR_SURFACE, { ogId: 'floor-surface' }), [0, 0, 0], 0xe5edf1);

const WALL_PROFILE = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 6, breadth: 0.2 }, { ogId: 'wall-profile' });
const WALL = new Solid(OG_OPERATION_EXTRUDE, { profile: WALL_PROFILE, distance: 3 }, { ogId: 'entry-wall' });
const OPENING = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.9, height: 2.1, depth: 0.4 }, { ogId: 'door-opening' });
OPENING.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 0] });
WALL.operate(OG_OPERATION_SUBTRACT, { tools: [OPENING] });
PLACE(WALL, [0, 0, -2.7], 0x7c9db3);
const SIDE_WALL = { width: 0.2, height: 2.45, depth: 4.9 };
PLACE(new Solid(OG_PRIMITIVE_CUBOID, SIDE_WALL, { ogId: 'side-wall' }), [-3, 0, 0], 0xa7bcc9);

const RAIL_PATH = new Wire(
  OG_PRIMITIVE_POLYLINE, { points: [[0, 1, 1], [4, 1, 1], [4, 1, 4]], closed: false }, { ogId: 'rail-path' },
);
const RAIL_PROFILE = new Wire(OG_PRIMITIVE_CIRCLE, { radius: 0.055 }, {
  ogId: 'rail-profile', plane: { origin: [0, 1, 1], normal: [1, 0, 0], xDirection: [0, 0, 1] },
});
const RAIL = new Solid(OG_OPERATION_SWEEP, { profile: RAIL_PROFILE, path: RAIL_PATH }, { ogId: 'swept-rail' });
PLACE(RAIL, [-2, 0, -2.2], 0x2476a5);

const POST_LOCATIONS: [number, number, number][] = [
  [-2, 0, -1.2], [-1, 0, -1.2], [0, 0, -1.2], [1, 0, -1.2], [2, 0, -1.2],
  [2, 0, -0.2], [2, 0, 0.8], [2, 0, 1.8],
];
const POST = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.055, height: 1 }, { ogId: 'rail-post-1' });
const POSTS = POST_LOCATIONS.map((location, index) => ({
  solid: index === 0 ? POST : POST.instance({ ogId: `rail-post-${String(index + 1)}` }),
  location,
}));
POSTS.forEach(({ solid, location }) => PLACE(solid, location, 0x2476a5));
LEVEL.addChild(SOLIDS);
OPENING.dispose();
WALL_PROFILE.dispose();
RAIL_PATH.dispose();
RAIL_PROFILE.dispose();

const GRID = new THREE.GridHelper(24, 24, 0xc3d1db, 0xd8e1e8);
GRID.position.y = -0.25;
SCENE.add(GRID);

function resetView(): void {
  const mobile = VIEWPORT.clientWidth < 600;
  const scale = mobile ? 1.45 : 1;
  CAMERA.position.set(8 * scale, 6 * scale, 10 * scale);
  CONTROLS.target.set(0, 0.85, 0);
  CONTROLS.update();
  render();
}

function resize(): void {
  fitRenderer(VIEWPORT, CAMERA, RENDERER);
  render();
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

const RESIZE_OBSERVER = new ResizeObserver(resize);
RESIZE_OBSERVER.observe(VIEWPORT);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => {
  STATUS.textContent = `Geometry error: ${String(error)}`;
});
RESET_BUTTON.addEventListener('click', resetView);
EDGES_BUTTON.addEventListener('click', () => {
  const enabled = EDGES_BUTTON.getAttribute('aria-pressed') !== 'true';
  EDGES_BUTTON.setAttribute('aria-pressed', String(enabled));
  EDGES_BUTTON.textContent = enabled ? 'Hide edges' : 'Show edges';
  SOLIDS.forEach((solid) => { solid.setAppearance({ outline: enabled }); });
  render();
});
async function exportLevel(): Promise<void> {
  EXPORT_BUTTON.disabled = true;
  STATUS.textContent = 'Exporting STEP…';
  try {
    const result = await OpenGeometry.exportStep({ nodes: [LEVEL], name: 'OpenGeometry 2.5 example' });
    const url = URL.createObjectURL(new Blob([result.text], { type: 'application/step' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = 'opengeometry-example.step';
    link.click();
    setTimeout(() => { URL.revokeObjectURL(url); }, 1000);
    STATUS.textContent = 'STEP exported';
  } catch (error) {
    STATUS.textContent = `Export failed: ${String(error)}`;
  } finally {
    EXPORT_BUTTON.disabled = false;
  }
}
EXPORT_BUTTON.addEventListener('click', () => { void exportLevel(); });

resize();
resetView();
const READY = await OpenGeometry.settled();
if (READY.failed.length) throw new Error(`${String(READY.failed.length)} solids failed to tessellate`);
render();
STATUS.textContent = `${String(SOLIDS.length)} solids ready · drag to orbit · scroll to zoom`;
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

(window as typeof window & { __ogExample?: unknown }).__ogExample = {
  camera: CAMERA, controls: CONTROLS, renderer: RENDERER, scene: SCENE, solids: SOLIDS, level: LEVEL,
  render, resetView,
  exportStep: () => OpenGeometry.exportStep({ nodes: [LEVEL] }),
};

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => undefined, () => {
  SOLIDS.slice().reverse().forEach((solid) => { solid.dispose(); });
  LEVEL.dispose();
});
