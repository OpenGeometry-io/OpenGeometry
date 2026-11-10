import * as THREE from 'three';
import {
  OpenGeometry, Solid,
  OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_ROUNDED_BOX, OG_TRANSFORM_TRANSLATE,
} from '../../../../dist/index.js';
import { activeBackend } from '../../../../dist/testing.js';
import { createRenderer, publishFixture, releasePage } from '../support/test-page';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });
const ERRORS: unknown[] = [];
OpenGeometry.on('error', (event) => { ERRORS.push(event); });

const PLATE = new Solid(OG_PRIMITIVE_ROUNDED_BOX, { width: 0.12, height: 0.01, depth: 0.08, radius: 0.01 });
const HOLES = [[-0.045, -0.025], [0.045, -0.025], [-0.045, 0.025], [0.045, 0.025]].map(([x, z]) => {
  const hole = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.004, height: 0.02 });
  hole.transform(OG_TRANSFORM_TRANSLATE, { offset: [x, -0.005, z] });
  return hole;
});
PLATE.operate(OG_OPERATION_SUBTRACT, { tools: HOLES });

const SCENE = new THREE.Scene();
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x444444, 2), PLATE);
const CAMERA = new THREE.PerspectiveCamera(50, innerWidth / innerHeight, 0.001, 10);
CAMERA.position.set(0.12, 0.1, 0.14);
CAMERA.lookAt(0, 0, 0);
const RENDERER = createRenderer(innerWidth, innerHeight);
RENDERER.render(SCENE, CAMERA);
const SETTLED = await OpenGeometry.settled();
RENDERER.render(SCENE, CAMERA);
RENDERER.setAnimationLoop(() => { RENDERER.render(SCENE, CAMERA); });

const STEP = await OpenGeometry.exportStep({ nodes: [PLATE], unit: 'millimetre', upAxis: 'Z' });

publishFixture('ogQuickStart', {
  plate: PLATE,
  bounds: PLATE.getBounds(),
  renderer: RENDERER,
  settled: SETTLED,
  report: STEP.report,
  text: STEP.text,
  errors: ERRORS,
  get backend() { return activeBackend(); },
  dispose: () => {
    RENDERER.setAnimationLoop(null);
    releasePage(RENDERER);
  },
}, 'Quick start ready');
