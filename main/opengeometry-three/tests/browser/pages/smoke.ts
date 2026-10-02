import * as THREE from 'three';
import { OpenGeometry, Solid, Wire, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_RECTANGLE } from '../../../../dist/index.js';
import { activeBackend, graph } from '../../../../dist/testing.js';
import type { SmokeFixture } from '../support/fixture-types';
import { bootKernel, createRenderer, publishFixture, releasePage, required } from '../support/test-page';
import { reservedNameCollisions } from './smoke/reserved-names';

const WORKER_URL = await bootKernel();
const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xf7f8fb);
SCENE.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
const LIGHT = new THREE.DirectionalLight(0xffffff, 2);
LIGHT.position.set(5, 8, 5);
SCENE.add(LIGHT);
const CAMERA = new THREE.PerspectiveCamera(60, 1, 0.1, 100);
CAMERA.position.set(3, 3, 5);
CAMERA.lookAt(0, 0.5, 0);
const RENDERER = createRenderer(320, 240);
const BODY = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'smoke-cube' });
const WIRE = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId: 'smoke-wire' });
SCENE.add(BODY);
RENDERER.render(SCENE, CAMERA);
const READY = await OpenGeometry.settled();
if (READY.failed.length) throw new Error('Cube geometry did not settle');
RENDERER.render(SCENE, CAMERA);
const RESERVED = reservedNameCollisions({ solid: BODY, wire: WIRE });
const WORKER_PROBES = [
  { kind: 'init', request: 0, module: 'invalid' }, { kind: 'nonsense', request: 7 }, { kind: 'snapshot', request: 8 },
];

function sceneHit(): ReturnType<SmokeFixture['resolveSceneHit']> {
  const origin = new THREE.Vector3(0.9, 0.5, 5);
  const ray = new THREE.Raycaster(origin, new THREE.Vector3(0.45, 0.5, 0.5).sub(origin).normalize());
  const hits = ray.intersectObjects(SCENE.children, true);
  const first = hits[0];
  return {
    hit: first && OpenGeometry.resolveHit(first),
    outlineHit: hits.some((hit) => hit.object === BODY.outline),
  };
}

function nextReply(worker: Worker, message: object): Promise<unknown> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => { resolve(undefined); }, 2_000);
    worker.onmessage = (event) => {
      clearTimeout(timer);
      resolve(event.data);
    };
    worker.postMessage(message);
  });
}

publishFixture('ogSmoke', {
  body: BODY, renderer: RENDERER, scene: SCENE, camera: CAMERA,
  threeRevision: THREE.REVISION,
  reservedNameCollisions: RESERVED.collisions,
  reservedNamesDerived: RESERVED.derived,
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
  resolveSceneHit: sceneHit,
  getInlineBuffers: () => {
    const info = JSON.parse(graph().node(BODY.ogId)) as { shapeId: string };
    return graph().buffers(info.shapeId, required(BODY.record, 'a cube record').bucket, 2_000_000);
  },
  workerReplies: async () => {
    const worker = new Worker(WORKER_URL, { type: 'module' });
    const replies: unknown[] = [];
    try {
      for (const message of WORKER_PROBES) replies.push(await nextReply(worker, message));
      return replies;
    } finally { worker.terminate(); }
  },
  dispose: () => {
    WIRE.dispose();
    BODY.dispose();
    releasePage(RENDERER);
  },
}, 'Kernel loaded');
