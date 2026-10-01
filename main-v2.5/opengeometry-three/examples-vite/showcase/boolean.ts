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
  OG_OPERATION_INTERSECT, OG_OPERATION_SUBTRACT, OG_OPERATION_UNION,
  OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';

const VIEWPORT = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
const OPERATION_INPUT = requiredElement('[data-control="operation"] [data-select]', HTMLSelectElement);
const OFFSET = numberControl('offset-x');
const REPORT = requiredElement('#result-report', HTMLElement);
const ERROR_READOUT = requiredElement('#error', HTMLElement);
await bootExample('./');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.GridHelper(30, 30, 0x4460ff, 0xd5ddff));
const CAMERA = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
VIEWPORT.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.enableDamping = false;
CONTROLS.minDistance = 4;
CONTROLS.maxDistance = 35;

type BooleanBodies = { host: Solid; tool: Solid; result: Solid };

let revision = 0;
let current: BooleanBodies | undefined;
let updateFrame = 0;

function disposeBodies(bodies: Solid[]): void { bodies.slice().reverse().forEach((body) => { body.dispose(); }); }

function createExample(): BooleanBodies {
  const id = ++revision;
  const created: Solid[] = [];
  try {
    const host = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 2, depth: 2 }, { ogId: `host-${String(id)}` });
    created.push(host);
    const tool = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 2, depth: 2 }, { ogId: `tool-${String(id)}` });
    created.push(tool);
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [Number(OFFSET.range.value), 0.4, 0.4] });
    const result = host.duplicate({ ogId: `result-${String(id)}` });
    created.push(result);
    const operation = OPERATION_INPUT.value === 'union' ? OG_OPERATION_UNION
      : OPERATION_INPUT.value === 'intersection' ? OG_OPERATION_INTERSECT : OG_OPERATION_SUBTRACT;
    result.operate(operation, { tools: [tool] });
    host.transform(OG_TRANSFORM_TRANSLATE, { offset: [-4, 0, -1] });
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [-4, 0, -1] });
    result.transform(OG_TRANSFORM_TRANSLATE, { offset: [3, 0, -1] });
    host.setAppearance({ color: 0xdc2626, opacity: 0.6, outline: true, deflection: 0.01 });
    tool.setAppearance({ color: 0xf97316, opacity: 0.6, outline: true, deflection: 0.01 });
    const override = OPERATION_INPUT.value === 'subtraction-override';
    result.setAppearance({
      color: override ? 0x22c55e : 0xdc2626, opacity: override ? 0.6 : 1, outline: true, deflection: 0.01,
    });
    return { host, tool, result };
  } catch (error) {
    disposeBodies(created);
    throw error;
  }
}

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resetView(): void {
  const narrow = VIEWPORT.clientWidth < 700;
  CAMERA.position.set(narrow ? 9 : 8, narrow ? 6 : 5, narrow ? 19 : 12);
  CONTROLS.target.set(-0.1, 1, 0);
  CONTROLS.update();
  render();
}

const RESIZE = narrowAwareResize(VIEWPORT, CAMERA, RENDERER, { resetView, render });

function rebuild(): void {
  let next: BooleanBodies | undefined;
  const started = performance.now();
  try {
    next = createExample();
    SCENE.add(next.host, next.tool, next.result);
    SCENE.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    if ([next.host, next.tool, next.result].some((body) => body.record?.revision !== body.lastInfo.shapeRevision)) {
      throw new Error('Boolean geometry did not finish');
    }
    const previous = current;
    current = next;
    if (previous) disposeBodies([previous.host, previous.tool, previous.result]);
    REPORT.textContent = JSON.stringify(next.result.getReport(), null, 2);
    ERROR_READOUT.textContent = '';
    const selected = OPERATION_INPUT.selectedOptions[0];
    if (!selected) throw new Error('No Boolean operation is selected');
    const faces = String(next.result.getBrep().topology.faces.length);
    const elapsed = (performance.now() - started).toFixed(1);
    STATUS.textContent = `${selected.text} ready · ${faces} faces · ${elapsed} ms`;
  } catch (error) {
    if (next && next !== current) disposeBodies([next.host, next.tool, next.result]);
    ERROR_READOUT.textContent = `Previous result retained. ${String(error)}`;
    STATUS.textContent = 'Boolean error';
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

OPERATION_INPUT.addEventListener('change', scheduleRebuild);
onNumberInput(OFFSET, () => { scheduleRebuild(); });
OFFSET.range.addEventListener('change', rebuildNow);
OFFSET.number.addEventListener('change', rebuildNow);

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

(window as typeof window & { __ogBooleanExample?: unknown }).__ogBooleanExample = {
  camera: CAMERA, controls: CONTROLS, renderer: RENDERER,
  get current() { return current; },
  render, rebuild,
};

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  if (current) disposeBodies([current.host, current.tool, current.result]);
});
