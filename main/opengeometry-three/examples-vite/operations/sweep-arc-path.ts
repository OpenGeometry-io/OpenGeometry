import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { latestOnly, numberControl, onNumberInput } from '../shared/controls';
import { requiredElement } from '../shared/dom';
import { startFpsMeter } from '../shared/fps';
import { bootExample } from '../shared/kernel';
import { releaseOnUnload } from '../shared/unload';
import { fitRenderer } from '../shared/viewport';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_SWEEP, OG_PRIMITIVE_POLYLINE, OG_PRIMITIVE_PROFILE,
} from '../../../dist/index.js';

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const ERROR = requiredElement('#error', HTMLElement);
const RADIUS = numberControl('radius');
const ANGLE = numberControl('angle');
const OUTLINE_INPUT = requiredElement('#outline', HTMLInputElement);
await bootExample('../');

const RUN = 2;
const HALF_THICKNESS = 0.15;
const HALF_HEIGHT = 0.55;
const SECTION_AREA = 4 * HALF_THICKNESS * HALF_HEIGHT;
const DEFLECTION = 1e-4;
const COLOR = 0x329ac7;

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
const GRID = new THREE.GridHelper(16, 16, 0x7188aa, 0xcbd5e1);
GRID.position.y = -HALF_HEIGHT;
SCENE.add(GRID);
const CAMERA = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
CAMERA.position.set(6, 6, 6);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(2, 0, -1.5);
CONTROLS.update();

type Point = [number, number];
type Segment = { to: Point; arc?: { center: Point; counterClockwise: boolean } };

function arcPath(radius: number, turn: number): { start: Point; segments: Segment[]; closed: false } {
  const center: Point = [RUN, radius];
  const bendEnd: Point = [center[0] + radius * Math.sin(turn), center[1] - radius * Math.cos(turn)];
  const end: Point = [bendEnd[0] + RUN * Math.cos(turn), bendEnd[1] + RUN * Math.sin(turn)];
  return {
    start: [0, 0],
    segments: [{ to: [RUN, 0] }, { to: bendEnd, arc: { center, counterClockwise: true } }, { to: end }],
    closed: false,
  };
}

function section(): [number, number, number][] {
  const [t, h] = [HALF_THICKNESS, HALF_HEIGHT];
  return [[0, -h, -t], [0, h, -t], [0, h, t], [0, -h, t]];
}

function isCylinder(surface: unknown): boolean {
  return typeof surface === 'object' && surface !== null && 'kind' in surface && surface.kind === 'Cylinder';
}

let generation = 0;
let path: Wire | undefined;
let profile: Wire | undefined;
let swept: Solid | undefined;
let retired: (Solid | Wire)[] = [];

function dropRetired(): void {
  retired.forEach((body) => {
    body.removeFromParent();
    body.dispose();
  });
  retired = [];
}

function release(): void {
  swept?.removeFromParent();
  swept?.dispose();
  path?.dispose();
  profile?.dispose();
  [swept, path, profile] = [undefined, undefined, undefined];
}

function build(radius: number, turn: number): void {
  retired = [swept, path, profile].filter((body) => body !== undefined);
  [swept, path, profile] = [undefined, undefined, undefined];
  generation += 1;
  const suffix = String(generation);
  path = new Wire(OG_PRIMITIVE_PROFILE, arcPath(radius, turn), { ogId: `arc-path-${suffix}` });
  profile = new Wire(OG_PRIMITIVE_POLYLINE, { points: section(), closed: true }, { ogId: `arc-section-${suffix}` });
  swept = new Solid(OG_OPERATION_SWEEP, { profile, path, caps: true }, { ogId: `arc-sweep-${suffix}` });
  swept.setAppearance({ color: COLOR, outline: OUTLINE_INPUT.checked });
  SCENE.add(swept);
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

function showStatus(radius: number, turn: number): void {
  if (!swept) throw new Error('The sweep was not built');
  const brep = swept.getBrep();
  const cylinders = brep.topology.faces.filter((face) => isCylinder(brep.geometry.surfaces[face.surface])).length;
  const length = 2 * RUN + radius * turn;
  const kernel = swept.measure({ deflection: DEFLECTION }).volume;
  STATUS.textContent = `Ready · ${String(brep.topology.faces.length)} faces (${String(cylinders)} cylinders)`
    + ` · path ${length.toFixed(6)} m · volume ${(SECTION_AREA * length).toFixed(6)} m³ (0.33 × path)`
    + ` · kernel ${kernel.toFixed(6)} m³`;
}

function refuse(error: unknown): void {
  release();
  dropRetired();
  render();
  ERROR.textContent = String(error);
  STATUS.textContent = 'Sweep refused';
}

async function update(): Promise<void> {
  ERROR.textContent = '';
  const radius = Number(RADIUS.range.value);
  const turn = Number(ANGLE.range.value) * Math.PI / 180;
  try {
    build(radius, turn);
    SCENE.updateMatrixWorld(true);
    const settled = await OpenGeometry.settled();
    const failure = settled.failed[0];
    if (failure) {
      refuse(failure.error);
      return;
    }
    dropRetired();
    render();
    showStatus(radius, turn);
  } catch (error) {
    refuse(error);
  }
}

const REQUEST_UPDATE = latestOnly(update);
onNumberInput(RADIUS, REQUEST_UPDATE);
onNumberInput(ANGLE, REQUEST_UPDATE);
OUTLINE_INPUT.addEventListener('change', () => {
  swept?.setAppearance({ outline: OUTLINE_INPUT.checked });
  render();
});

const RESIZE_OBSERVER = new ResizeObserver(resize);
RESIZE_OBSERVER.observe(APP);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => {
  ERROR.textContent = `Geometry error: ${String(error)}`;
});
resize();
await update();
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => undefined, release);
