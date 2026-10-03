import * as THREE from 'three';
import {
  OpenGeometry, Solid, Wire,
  OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_RECTANGLE, OG_TRANSFORM_TRANSLATE,
} from '../../../../dist/index.js';
import { activeBackend } from '../../../../dist/testing.js';
import { createRenderer, publishFixture, releasePage } from '../support/test-page';

await OpenGeometry.create({ wasmURL: '/opengeometry_bg.wasm' }, { workerURL: '/tessellation-worker.js' });
const ERRORS: unknown[] = [];
OpenGeometry.on('error', (event) => { ERRORS.push(event); });

const PROFILE = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 6, breadth: 0.2 });
const WALL = new Solid(OG_OPERATION_EXTRUDE, { profile: PROFILE, distance: 3 });
const DOOR = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.9, height: 2.1, depth: 0.4 });
DOOR.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 0] });
WALL.operate(OG_OPERATION_SUBTRACT, { tools: [DOOR] });

const SCENE = new THREE.Scene();
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x444444, 2), WALL);
const CAMERA = new THREE.PerspectiveCamera(50, innerWidth / innerHeight, 0.1, 100);
CAMERA.position.set(6, 5, 8);
CAMERA.lookAt(0, 1.5, 0);
const RENDERER = createRenderer(innerWidth, innerHeight);
RENDERER.render(SCENE, CAMERA);
const SETTLED = await OpenGeometry.settled();
RENDERER.render(SCENE, CAMERA);
RENDERER.setAnimationLoop(() => { RENDERER.render(SCENE, CAMERA); });

const STEP = await OpenGeometry.exportStep({ nodes: [WALL], unit: 'millimetre', upAxis: 'Z' });

publishFixture('ogQuickStart', {
  wall: WALL,
  bounds: WALL.getBounds(),
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
