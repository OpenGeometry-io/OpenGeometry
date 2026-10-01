import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { requiredElement } from '../shared/dom.js';
import { startFpsMeter } from '../shared/fps.js';
import { bootExample } from '../shared/kernel.js';
import { releaseOnUnload } from '../shared/unload.js';
import { fitRenderer } from '../shared/viewport.js';
import {
  OGError, OpenGeometry, Solid,
  OG_OPERATION_INTERSECT, OG_OPERATION_SUBTRACT, OG_OPERATION_UNION,
  OG_PRIMITIVE_CYLINDER, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';

const VIEWPORT = requiredElement('#app', HTMLDivElement);
const OPERATION_INPUT = requiredElement('#operation', HTMLSelectElement);
const OFFSET_INPUT = requiredElement('#offset', HTMLInputElement);
const DEFLECTION_INPUT = requiredElement('#deflection', HTMLInputElement);
const REPORT = requiredElement('#report', HTMLElement);
const ERROR_READOUT = requiredElement('#error', HTMLElement);
const STATUS = requiredElement('#status', HTMLElement);
await bootExample('../');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.GridHelper(24, 24, 0x7188aa, 0xcbd5e1));
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
const CAMERA = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
CAMERA.position.set(9, 5, 9);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
VIEWPORT.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(0, 1, 0);
CONTROLS.enableDamping = true;

type Example = { host: Solid; tool: Solid; result: Solid | undefined; faces: number; contacts: number };
let displayed: Example | undefined;
let revision = 0;
let updateFrame = 0;
let updateGeometry = true;

const CHORD_ERROR = (): number => 10 ** Number(DEFLECTION_INPUT.value);
const OPERATION = (): string => OPERATION_INPUT.value === 'union' ? OG_OPERATION_UNION
  : OPERATION_INPUT.value === 'intersection' ? OG_OPERATION_INTERSECT : OG_OPERATION_SUBTRACT;

function bodies(example: Example): Solid[] {
  return [example.host, example.tool, example.result].filter((body): body is Solid => Boolean(body));
}
function dispose(example: Example): void { bodies(example).reverse().forEach((body) => { body.dispose(); }); }
function ready(example: Example): void {
  SCENE.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  for (const body of bodies(example)) {
    if (body.record?.revision !== body.lastInfo.shapeRevision) throw new Error('Boolean geometry did not finish');
  }
}

function createExample(): Example {
  const id = ++revision;
  const created: Solid[] = [];
  try {
    const host = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 1, height: 2.4 }, { ogId: `analytic-host-${String(id)}` });
    created.push(host);
    const tool = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 1, height: 2.4 }, { ogId: `analytic-tool-${String(id)}` });
    created.push(tool);
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [Number(OFFSET_INPUT.value), 0, 0] });
    const result = host.duplicate({ ogId: `analytic-result-${String(id)}` });
    created.push(result);
    let faces = 0;
    let contacts = 0;
    let empty = false;
    try {
      result.operate(OPERATION(), { tools: [tool] });
      faces = result.getBrep().topology.faces.length;
      contacts = result.getReport().report.contacts.length;
    } catch (error) {
      if (!(error instanceof OGError && error.code === 'EmptyResult')) throw error;
      empty = true;
    }
    host.transform(OG_TRANSFORM_TRANSLATE, { offset: [-3.5, 0, 0] });
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [-3.5, 0, 0] });
    if (!empty) result.transform(OG_TRANSFORM_TRANSLATE, { offset: [3.5, 0, 0] });
    host.setAppearance({ color: 0x3b82f6, opacity: 0.65, outline: true, deflection: CHORD_ERROR() });
    tool.setAppearance({ color: 0xf97316, opacity: 0.65, outline: true, deflection: CHORD_ERROR() });
    if (!empty) result.setAppearance({ color: 0x22c55e, outline: true, deflection: CHORD_ERROR() });
    if (empty) { result.dispose(); created.pop(); }
    return { host, tool, result: empty ? undefined : result, faces, contacts };
  } catch (error) {
    created.reverse().forEach((body) => { body.dispose(); });
    throw error;
  }
}

function rebuild(): void {
  const started = performance.now();
  let next: Example | undefined;
  try {
    next = createExample();
    for (const body of bodies(next)) SCENE.add(body);
    ready(next);
    const previous = displayed;
    displayed = next;
    if (previous) dispose(previous);
    const counts = `${String(next.faces)} result faces · ${String(next.contacts)} contacts`;
    REPORT.textContent = `${OPERATION_INPUT.value} · analytic\n${counts}`;
    ERROR_READOUT.textContent = '';
    STATUS.textContent = `Ready · ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) {
    if (next) dispose(next);
    ERROR_READOUT.textContent = `Previous result retained. ${String(error)}`;
    STATUS.textContent = 'Boolean error';
  }
}

function retessellate(): void {
  if (!displayed) { rebuild(); return; }
  const started = performance.now();
  try {
    for (const body of bodies(displayed)) body.setAppearance({ deflection: CHORD_ERROR() });
    ready(displayed);
    ERROR_READOUT.textContent = '';
    STATUS.textContent = `Ready · mesh ${(performance.now() - started).toFixed(1)} ms`;
  } catch (error) {
    ERROR_READOUT.textContent = String(error);
    STATUS.textContent = 'Tessellation error';
  }
}

function schedule(geometry: boolean): void {
  updateGeometry ||= geometry;
  if (updateFrame) return;
  updateFrame = requestAnimationFrame(() => {
    updateFrame = 0;
    if (updateGeometry) rebuild(); else retessellate();
    updateGeometry = false;
  });
}

OPERATION_INPUT.addEventListener('change', () => { schedule(true); });
OFFSET_INPUT.addEventListener('input', () => { schedule(true); });
DEFLECTION_INPUT.addEventListener('input', () => { schedule(false); });

const RESIZE_OBSERVER = new ResizeObserver(() => { fitRenderer(VIEWPORT, CAMERA, RENDERER); });
RESIZE_OBSERVER.observe(VIEWPORT);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => { ERROR_READOUT.textContent = String(error); });
rebuild();
updateGeometry = false;
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

(window as typeof window & { __ogAnalyticBooleanExample?: unknown }).__ogAnalyticBooleanExample = {
  camera: CAMERA, controls: CONTROLS, renderer: RENDERER,
  get current() { return displayed; },
  rebuild,
};

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => { if (updateFrame) window.cancelAnimationFrame(updateFrame); }, () => {
  if (displayed) dispose(displayed);
});
