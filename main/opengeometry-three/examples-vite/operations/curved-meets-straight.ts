import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { latestOnly, numberControl, onNumberInput } from '../shared/controls';
import { requiredElement } from '../shared/dom';
import { startFpsMeter } from '../shared/fps';
import { bootExample } from '../shared/kernel';
import { releaseOnUnload } from '../shared/unload';
import { fitRenderer } from '../shared/viewport';
import {
  OpenGeometry, Solid, Wire,
  OG_OPERATION_EXTRUDE, OG_OPERATION_UNION, OG_PRIMITIVE_PROFILE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const ERROR = requiredElement('#error', HTMLElement);
const REACH = numberControl('reach');
const OUTLINE_INPUT = requiredElement('#outline', HTMLInputElement);
await bootExample('../');

const INNER = 2.9;
const OUTER = 3.1;
const HEIGHT = 3;
const RUN = 2;
const SIDE_REACH = 0.05;
const SIDE_BACK = 0.2;
const SPREAD = 7;
const DEFLECTION = 1e-4;
const COLOR = 0x329ac7;
const INPUT_COLOR = 0xf97316;

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
SCENE.add(new THREE.GridHelper(20, 20, 0x7188aa, 0xcbd5e1));
const CAMERA = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
CAMERA.position.set(8, 8, 8);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(1, 1.5, -1);
CONTROLS.update();

type Point = [number, number];
type Segment = { to: Point; arc?: { center: Point; counterClockwise: boolean } };
type Section = { start: Point; segments: Segment[]; closed: true };

function quarterSection(): Section {
  return {
    start: [INNER, 0],
    segments: [
      { to: [OUTER, 0] },
      { to: [0, OUTER], arc: { center: [0, 0], counterClockwise: true } },
      { to: [0, INNER] },
      { to: [INNER, 0], arc: { center: [0, 0], counterClockwise: false } },
    ],
    closed: true,
  };
}

function rectangleSection(low: Point, high: Point): Section {
  return {
    start: low,
    segments: [{ to: [high[0], low[1]] }, { to: high }, { to: [low[0], high[1]] }, { to: low }],
    closed: true,
  };
}

function overlapArea(reach: number): number {
  const square = OUTER * OUTER;
  return (reach * Math.sqrt(square - reach * reach) + square * Math.asin(reach / OUTER)) / 2 - INNER * reach;
}

function expectedVolume(reach: number): number {
  const quarter = Math.PI / 4 * (OUTER * OUTER - INNER * INNER);
  const tangent = (OUTER - INNER) * (RUN + reach);
  const side = (SIDE_BACK + SIDE_REACH) * RUN;
  return HEIGHT * (quarter + tangent + side - overlapArea(reach) - overlapArea(SIDE_REACH));
}

let generation = 0;
let bodies: (Solid | Wire)[] = [];
let united: Solid | undefined;
let retired: (Solid | Wire)[] = [];

function dropRetired(): void {
  retired.reverse().forEach((body) => {
    body.removeFromParent();
    body.dispose();
  });
  retired = [];
}

function release(): void {
  united?.removeFromParent();
  bodies.reverse().forEach((body) => { body.dispose(); });
  [bodies, united] = [[], undefined];
}

function prism(section: Section, name: string): Solid {
  const suffix = String(generation);
  const profile = new Wire(OG_PRIMITIVE_PROFILE, section, { ogId: `${name}-section-${suffix}` });
  bodies.push(profile);
  const solid = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: HEIGHT }, { ogId: `${name}-${suffix}` });
  bodies.push(solid);
  solid.setAppearance({ color: INPUT_COLOR, opacity: 0.65, outline: true });
  SCENE.add(solid);
  return solid;
}

function build(reach: number): void {
  [retired, bodies, united] = [bodies, [], undefined];
  generation += 1;
  const quarter = prism(quarterSection(), 'quarter');
  const tangent = prism(rectangleSection([INNER, -RUN], [OUTER, reach]), 'tangent');
  const side = prism(rectangleSection([-SIDE_BACK, INNER], [SIDE_REACH, INNER + RUN]), 'side');
  united = quarter.duplicate({ ogId: `united-${String(generation)}` });
  bodies.push(united);
  united.operate(OG_OPERATION_UNION, { tools: [tangent, side] });
  for (const input of [quarter, tangent, side]) input.transform(OG_TRANSFORM_TRANSLATE, { offset: [-SPREAD, 0, 0] });
  united.setAppearance({ color: COLOR, outline: OUTLINE_INPUT.checked });
  SCENE.add(united);
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

function showStatus(reach: number): void {
  if (!united) throw new Error('The union was not built');
  const faces = united.getBrep().topology.faces.length;
  const segments = united.outline.geometry.getAttribute('position').count / 2;
  const measured = united.measure({ deflection: DEFLECTION });
  const { valid, closed } = united.validity();
  STATUS.textContent = `Ready · ${String(faces)} faces · ${String(segments)} outline segments (whole body)`
    + ` · volume ${expectedVolume(reach).toFixed(6)} m³ (sum − overlap) · kernel ${measured.volume.toFixed(6)} m³`
    + ` · bound ${measured.errorBound.toExponential(2)} · valid ${String(valid)} · closed ${String(closed)}`;
}

async function update(): Promise<void> {
  ERROR.textContent = '';
  const reach = Number(REACH.range.value);
  try {
    build(reach);
    SCENE.updateMatrixWorld(true);
    const settled = await OpenGeometry.settled();
    const failure = settled.failed[0];
    if (failure) throw new Error(String(failure.error));
    dropRetired();
    render();
    showStatus(reach);
  } catch (error) {
    release();
    dropRetired();
    render();
    ERROR.textContent = String(error);
    STATUS.textContent = 'Union failed';
  }
}

const REQUEST_UPDATE = latestOnly(update);
onNumberInput(REACH, REQUEST_UPDATE);
OUTLINE_INPUT.addEventListener('change', () => {
  united?.setAppearance({ outline: OUTLINE_INPUT.checked });
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
