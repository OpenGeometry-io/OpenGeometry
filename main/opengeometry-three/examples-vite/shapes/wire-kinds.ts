import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { requiredElement } from '../shared/dom';
import { startFpsMeter } from '../shared/fps';
import { bootExample } from '../shared/kernel';
import { releaseOnUnload } from '../shared/unload';
import { fitRenderer } from '../shared/viewport';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_EXTRUDE, OG_PRIMITIVE_PARAMS_REGULAR_POLYGON,
  OG_PRIMITIVE_PARAMS_ROUNDED_RECTANGLE, OG_PRIMITIVE_PARAMS_SLOT, OG_PRIMITIVE_REGULAR_POLYGON,
  OG_PRIMITIVE_ROUNDED_RECTANGLE, OG_PRIMITIVE_SLOT,
} from '../../../dist/index.js';

const APP = requiredElement('#app', HTMLDivElement);
const STATUS = requiredElement('#status', HTMLElement);
await bootExample('../');

const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xeef2ff);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
SCENE.add(new THREE.GridHelper(10, 20, 0x4460ff, 0xd5ddff));
const CAMERA = new THREE.PerspectiveCamera(50, 1, 0.05, 100);
CAMERA.position.set(0, 2.6, 3.2);
const RENDERER = new THREE.WebGLRenderer({ antialias: true });
RENDERER.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
APP.append(RENDERER.domElement);
const CONTROLS = new OrbitControls(CAMERA, RENDERER.domElement);
CONTROLS.target.set(0, 0.2, 0);
CONTROLS.update();

const WIRES = [
  new Wire(OG_PRIMITIVE_REGULAR_POLYGON, OG_PRIMITIVE_PARAMS_REGULAR_POLYGON({ sides: 6, radius: 0.45 }), {
    ogId: 'regular-polygon', plane: { origin: [-1.4, 0, 0] },
  }),
  new Wire(OG_PRIMITIVE_SLOT, OG_PRIMITIVE_PARAMS_SLOT({ length: 0.6, width: 0.4 }), { ogId: 'slot' }),
  new Wire(OG_PRIMITIVE_ROUNDED_RECTANGLE, OG_PRIMITIVE_PARAMS_ROUNDED_RECTANGLE({
    width: 0.9, breadth: 0.6, radius: 0.12,
  }), { ogId: 'rounded-rectangle', plane: { origin: [1.4, 0, 0] } }),
];
const PRISMS = WIRES.map((profile) => new Solid(
  OG_OPERATION_EXTRUDE, { profile, distance: 0.3 }, { ogId: `${profile.ogId}-prism` },
));
for (const prism of PRISMS) {
  prism.setAppearance({ color: 0x0ea5e9, outline: true, opacity: 0.6 });
  SCENE.add(prism);
}
for (const wire of WIRES) SCENE.add(wire);

function render(): void { RENDERER.render(SCENE, CAMERA); }

function resize(): void {
  fitRenderer(APP, CAMERA, RENDERER);
  render();
}

const RESIZE_OBSERVER = new ResizeObserver(resize);
RESIZE_OBSERVER.observe(APP);
CONTROLS.addEventListener('start', () => { OpenGeometry.setCameraMotion(true); });
CONTROLS.addEventListener('end', () => { OpenGeometry.setCameraMotion(false); render(); });
const UNSUBSCRIBE_ERROR = OpenGeometry.on('error', (error) => {
  STATUS.textContent = `Geometry error: ${String(error)}`;
});
resize();
const READY = await OpenGeometry.settled();
if (READY.failed.length) throw new Error('Wire kinds geometry failed');
render();
const EDGES = WIRES.map((wire) => wire.getBrep().topology.edges.length).join(', ');
STATUS.textContent = `Wire kinds ready · ${EDGES} edges`;
const STOP_FPS_METER = startFpsMeter(RENDERER, SCENE, CAMERA, CONTROLS);

releaseOnUnload({
  resizeObserver: RESIZE_OBSERVER, stopFpsMeter: STOP_FPS_METER, unsubscribeError: UNSUBSCRIBE_ERROR,
  controls: CONTROLS, renderer: RENDERER,
}, () => undefined, () => {
  for (const body of [...PRISMS, ...WIRES]) body.dispose();
});
