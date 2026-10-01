import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID } from '../../../../dist/index.js';
import { activeBackend, graph } from '../../../../dist/testing.js';
import { bootKernel, createRenderer, publishFixture, releasePage, required } from '../support/test-page.js';

const WORKER_URL = await bootKernel();
const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xf7f8fb);
const CAMERA = new THREE.PerspectiveCamera(60, 1, 0.1, 100);
CAMERA.position.set(3, 3, 5);
CAMERA.lookAt(0, 0.5, 0);
const RENDERER = createRenderer(320, 240);
const BODY = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'smoke-cube' });
SCENE.add(BODY);
RENDERER.render(SCENE, CAMERA);
const READY = await OpenGeometry.settled();
if (READY.failed.length) throw new Error('Cube geometry did not settle');
RENDERER.render(SCENE, CAMERA);
const RESERVED = new Set<string>();
for (let object: object | null = new THREE.Group(); object; object = Reflect.getPrototypeOf(object)) {
  for (const key of Object.getOwnPropertyNames(object)) RESERVED.add(key);
}
const ADDED_NAMES = [
  'ogId', 'handle', 'generation', 'bodyType', 'appearance', 'lastInfo', 'record', 'inLimbo', 'surface', 'outline',
  'transform', 'getPlacement', 'getWorldPlacement', 'addChild', 'removeChild', 'getChildren', 'getParent', 'getBounds',
  'getBrep', 'rebuild', 'operate', 'instance', 'duplicate', 'makeUnique', 'getInstanceCount', 'setAppearance',
  'getReport', 'dispose',
];

publishFixture({
  body: BODY, renderer: RENDERER, scene: SCENE, camera: CAMERA,
  threeRevision: THREE.REVISION,
  reservedNameCollisions: ADDED_NAMES.filter((name) => RESERVED.has(name)),
  get backend() { return activeBackend(); },
  render: () => { RENDERER.render(SCENE, CAMERA); },
  settled: () => OpenGeometry.settled(),
  exportStep: () => OpenGeometry.exportStep({ nodes: [BODY] }),
  resolveHit: () => {
    const ray = new THREE.Raycaster();
    ray.set(new THREE.Vector3(2, 0.5, 5), new THREE.Vector3(0, 0, -1));
    const hit = ray.intersectObject(BODY.surface)[0];
    return hit && OpenGeometry.resolveHit(hit);
  },
  getInlineBuffers: () => {
    const info = JSON.parse(graph().node(BODY.ogId)) as { shapeId: string };
    return graph().buffers(info.shapeId, required(BODY.record, 'a cube record').bucket, 2_000_000);
  },
  workerInitError: async () => {
    const worker = new Worker(WORKER_URL, { type: 'module' });
    try {
      return await new Promise<unknown>((resolve, reject) => {
        worker.onmessage = (event) => { resolve(event.data); };
        worker.onerror = (event) => { reject(new Error(event.message)); };
        worker.postMessage({ kind: 'init', request: 0, module: 'invalid' });
      });
    } finally { worker.terminate(); }
  },
  dispose: () => { BODY.dispose(); releasePage(RENDERER); },
}, 'Kernel loaded');
