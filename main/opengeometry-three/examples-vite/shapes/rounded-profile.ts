import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { latestOnly, numberControl, onNumberInput } from '../shared/controls';
import { requiredElement } from '../shared/dom';
import { startFpsMeter } from '../shared/fps';
import { bootExample } from '../shared/kernel';
import { releaseOnUnload } from '../shared/unload';
import { fitRenderer } from '../shared/viewport';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_EXTRUDE, OG_PRIMITIVE_PROFILE,
} from '../../../dist/index.js';

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const ERROR = requiredElement('#error', HTMLElement);
const RADIUS = numberControl('radius');
const OUTLINE_INPUT = requiredElement('#outline', HTMLInputElement);
await bootExample('../');

const HALF_WIDTH = 2;
const HALF_HEIGHT = 1;
const DISTANCE = 3;
const DEFLECTION = 2.5e-7;
const COLOR = 0x329ac7;

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
SCENE.add(new THREE.GridHelper(16, 16, 0x7188aa, 0xcbd5e1));
const CAMERA = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
CAMERA.position.set(7, 5, 7);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(0, 1, 0);
CONTROLS.update();

type Point = [number, number];
type Segment = { to: Point; arc?: { center: Point; counterClockwise: boolean } };

function corner(to: Point, center: Point, radius: number): Segment[] {
  return radius > 0 ? [{ to, arc: { center, counterClockwise: true } }] : [];
}

function roundedProfile(radius: number): { start: Point; segments: Segment[]; closed: true } {
  const [w, h, r] = [HALF_WIDTH, HALF_HEIGHT, radius];
  return {
    start: [-w + r, -h],
    segments: [
      { to: [w - r, -h] },
      ...corner([w, -h + r], [w - r, -h + r], r),
      ...(r < h ? [{ to: [w, h - r] as Point }] : []),
      ...corner([w - r, h], [w - r, h - r], r),
      { to: [-w + r, h] },
      ...corner([-w, h - r], [-w + r, h - r], r),
      ...(r < h ? [{ to: [-w, -h + r] as Point }] : []),
      ...corner([-w + r, -h], [-w + r, -h + r], r),
    ],
    closed: true,
  };
}

function analyticVolume(radius: number): number {
  return (4 * HALF_WIDTH * HALF_HEIGHT - (4 - Math.PI) * radius * radius) * DISTANCE;
}

let generation = 0;
let profile: Wire | undefined;
let prism: Solid | undefined;
let retired: (Solid | Wire)[] = [];

function dropRetired(): void {
  retired.forEach((body) => {
    body.removeFromParent();
    body.dispose();
  });
  retired = [];
}

function build(): void {
  retired = [prism, profile].filter((body) => body !== undefined);
  [prism, profile] = [undefined, undefined];
  generation += 1;
  const suffix = String(generation);
  const section = roundedProfile(Number(RADIUS.range.value));
  profile = new Wire(OG_PRIMITIVE_PROFILE, section, { ogId: `rounded-profile-${suffix}` });
  prism = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: DISTANCE }, { ogId: `rounded-prism-${suffix}` });
  prism.setAppearance({ color: COLOR, outline: OUTLINE_INPUT.checked });
  SCENE.add(prism);
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

function showStatus(prefix: string): void {
  if (!prism) throw new Error('The prism was not built');
  const radius = Number(RADIUS.range.value);
  const faces = prism.getBrep().topology.faces.length;
  const kernel = prism.measure({ deflection: DEFLECTION }).volume;
  const analytic = analyticVolume(radius).toFixed(6);
  const measured = kernel.toFixed(6);
  STATUS.textContent = `${prefix} · ${String(faces)} faces · volume ${analytic} m³ (analytic) · kernel ${measured} m³`;
}

async function update(): Promise<void> {
  ERROR.textContent = '';
  try {
    build();
    SCENE.updateMatrixWorld(true);
    const settled = await OpenGeometry.settled();
    if (settled.failed.length) throw new Error('Rounded section geometry failed');
    dropRetired();
    render();
    showStatus('Ready');
  } catch (error) {
    dropRetired();
    ERROR.textContent = String(error);
    STATUS.textContent = 'Rounded section error';
  }
}

const REQUEST_UPDATE = latestOnly(update);
onNumberInput(RADIUS, REQUEST_UPDATE);
OUTLINE_INPUT.addEventListener('change', () => {
  prism?.setAppearance({ outline: OUTLINE_INPUT.checked });
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
}, () => undefined, () => {
  prism?.dispose();
  profile?.dispose();
});
