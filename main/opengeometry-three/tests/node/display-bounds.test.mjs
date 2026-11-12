import assert from 'node:assert/strict';
import * as NODE_TEST from 'node:test';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, OG_TRANSFORM_TRANSLATE } from '../../../dist/index.js';
import * as TESTING from '../../../dist/testing.js';
import { boot, cuboid } from './support.mjs';

const RENDERER = { info: { render: { frame: 0 } }, getDrawingBufferSize: (target) => target.set(1000, 1000) };
const TIMERS = Reflect.get(NODE_TEST, 'mock').timers;
const BODIES = 6;
const FRAMES = 8;

async function fresh() {
  await boot();
  OpenGeometry.reset();
  TIMERS.enable({ apis: ['setTimeout', 'Date'] });
}

function boundsLog() {
  const target = Object(TESTING.graph());
  const read = target.bounds;
  const log = { calls: 0, stop: () => { delete target.bounds; } };
  target.bounds = (...rest) => { log.calls++; return Reflect.apply(read, target, rest); };
  return log;
}

function move(body, offset) {
  body.transform(OG_TRANSFORM_TRANSLATE, { offset });
  OpenGeometry.flush();
  return body;
}

function gridScene() {
  const bodies = Array.from({ length: BODIES }, (_, index) => move(
    cuboid(`grid-${String(index)}`, 1 + index / 10),
    [3 * (index % 3), 0, 3 * Math.floor(index / 3)],
  ));
  const scene = new THREE.Scene();
  scene.add(...bodies);
  return { scene, bodies };
}

function orbit(camera, step) {
  const angle = step * 0.3;
  camera.position.set(3 + 12 * Math.cos(angle), 6, 1.5 + 12 * Math.sin(angle));
  camera.lookAt(3, 0, 1.5);
  camera.updateMatrixWorld();
  return camera;
}

async function frame(scene, camera) {
  TIMERS.tick(100);
  scene.updateMatrixWorld(true);
  RENDERER.info.render.frame++;
  scene.traverseVisible((object) => {
    if (object instanceof THREE.Mesh) Reflect.apply(object.onBeforeRender, object, [RENDERER, scene, camera]);
  });
  await Promise.resolve();
}

function uncached() {
  const state = Object(TESTING.runtime());
  state.displayBounds?.clear();
  if (state.lod) state.lod.stale = true;
}

async function orbitRun(options) {
  await fresh();
  const { scene, bodies } = gridScene();
  const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 1000);
  const log = boundsLog();
  const reads = [];
  const wanted = [];
  try {
    for (let step = 0; step < FRAMES; step++) {
      if (step === options.moved) move(bodies[1], [0, 0, -5]);
      if (options.uncached) uncached();
      const before = log.calls;
      await frame(scene, orbit(camera, step));
      reads.push(log.calls - before);
      wanted.push(bodies.map((body) => TESTING.wanted(body)));
    }
    TIMERS.tick(200);
    return { reads, wanted, total: log.calls, final: bodies.map((body) => TESTING.wanted(body)) };
  } finally {
    log.stop();
    TIMERS.reset();
    for (const body of bodies) body.dispose();
  }
}

test('an orbit of N frames over M bodies reads each body bounds once', async () => {
  const { total } = await orbitRun({});
  assert.equal(total, BODIES);
});

test('a transform makes the next pass re-read bounds and the buckets equal the uncached run', async () => {
  const cached = await orbitRun({ moved: 4 });
  const expected = Array.from({ length: FRAMES }, (_, step) => (step === 0 || step === 4 ? BODIES : 0));
  assert.deepEqual(cached.reads, expected);
  const plain = await orbitRun({ moved: 4, uncached: true });
  assert.deepEqual(cached.wanted, plain.wanted);
  assert.deepEqual(cached.final, plain.final);
});

async function settledGrid() {
  await fresh();
  const { scene, bodies } = gridScene();
  const camera = orbit(new THREE.PerspectiveCamera(60, 1, 0.1, 1000), 2);
  await frame(scene, camera);
  TIMERS.tick(200);
  TESTING.runtime().displayBounds?.clear();
  return { scene, bodies, camera };
}

test('a static camera with nothing newly displayed runs no further pass', async () => {
  const { scene, bodies, camera } = await settledGrid();
  const log = boundsLog();
  try {
    const before = bodies.map((body) => TESTING.wanted(body));
    for (let step = 0; step < 5; step++) await frame(scene, camera);
    assert.equal(log.calls, 0);
    assert.deepEqual(bodies.map((body) => TESTING.wanted(body)), before);
  } finally {
    log.stop();
    TIMERS.reset();
    for (const body of bodies) body.dispose();
  }
});

test('a body shown for the first time after a static period gets its target on the next frame', async () => {
  const { scene, bodies, camera } = await settledGrid();
  const twin = bodies[0];
  const late = cuboid('late', 1);
  late.visible = false;
  scene.add(late);
  try {
    for (let step = 0; step < 3; step++) await frame(scene, camera);
    const shown = TESTING.wanted(twin);
    assert.notEqual(TESTING.wanted(late), shown);
    late.visible = true;
    await frame(scene, camera);
    assert.equal(TESTING.wanted(late), shown);
  } finally {
    TIMERS.reset();
    late.dispose();
    for (const body of bodies) body.dispose();
  }
});

test('a body hidden during a static period marks the pass stale', async () => {
  await fresh();
  const near = move(cuboid('near', 1), [0, 0, -3]);
  const far = move(near.instance({ ogId: 'far' }), [0, 0, -300]);
  const scene = new THREE.Scene();
  scene.add(near, far);
  const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 1000);
  camera.updateMatrixWorld();
  const log = boundsLog();
  try {
    await frame(scene, camera);
    TIMERS.tick(200);
    const shared = TESTING.wanted(far);
    const reads = log.calls;
    for (let step = 0; step < 3; step++) await frame(scene, camera);
    assert.equal(log.calls, reads);
    near.visible = false;
    await frame(scene, camera);
    assert.equal(Number(TESTING.wanted(far)) > Number(shared), true);
  } finally {
    log.stop();
    TIMERS.reset();
    far.dispose();
    near.dispose();
  }
});
