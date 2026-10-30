import assert from 'node:assert/strict';
import * as NODE_TEST from 'node:test';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, OGError, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_ROTATE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import * as TESTING from '../../../dist/testing.js';
import { boot, cuboid, listen } from './support.mjs';

const RENDERER = { getDrawingBufferSize: (target) => target.set(1000, 1000) };
const P = 2 ** -10;
const TIMERS = Reflect.get(NODE_TEST, 'mock').timers;

async function fresh() {
  await boot();
  OpenGeometry.reset();
}

function aim(camera, target) {
  camera.zoom = 1 / (1000 * target);
  camera.updateProjectionMatrix();
  camera.updateMatrixWorld();
  return camera;
}

function ortho(target) {
  return aim(new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000), target);
}

async function frame(scene, camera) {
  scene.updateMatrixWorld(true);
  scene.traverseVisible((object) => {
    if (object instanceof THREE.Mesh) Reflect.apply(object.onBeforeRender, object, [RENDERER, scene, camera]);
  });
  await Promise.resolve();
}

function computeLog() {
  const provider = Object(TESTING.runtime().provider);
  const compute = provider.compute.bind(provider);
  const requests = [];
  provider.compute = (request) => { requests.push(request); return compute(request); };
  return { requests, stop: () => { delete provider.compute; } };
}

function kernelBuckets(body) {
  return JSON.parse(TESTING.graph().displayBuckets(body.ogId));
}

function pinWantedBucket(shapeId, bucket) {
  Reflect.get(TESTING, 'pinWantedBucket')(shapeId, bucket);
}

test('a body removed from the scene is not computed', async () => {
  await fresh();
  const scene = new THREE.Scene();
  const body = cuboid('removed-cube');
  const other = cuboid('kept-cube', 2);
  scene.add(body, other);
  const log = computeLog();
  const mine = () => log.requests.filter((request) => request.shapeId !== other.lastInfo.shapeId).length;
  try {
    scene.updateMatrixWorld(true);
    await Promise.resolve();
    const before = mine();
    scene.remove(body);
    scene.updateMatrixWorld(true);
    await Promise.resolve();
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 1.5, height: 1, depth: 1 });
    OpenGeometry.flush({ geometry: 'sync' });
    const { failed } = await OpenGeometry.settled();
    assert.equal(mine(), before);
    assert.deepEqual(failed, []);
  } finally {
    log.stop();
    other.dispose();
    body.dispose();
  }
});

test('a hidden rebuilt body is not reported failed', async () => {
  await fresh();
  const scene = new THREE.Scene();
  const body = cuboid('hidden-cube');
  scene.add(body);
  try {
    scene.updateMatrixWorld(true);
    await Promise.resolve();
    assert.deepEqual((await OpenGeometry.settled()).failed, []);
    body.visible = false;
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 1.5, height: 1, depth: 1 });
    assert.deepEqual((await OpenGeometry.settled()).failed, []);
  } finally {
    body.dispose();
  }
});

test("settled's deflection is temporary and checks the wanted key", async () => {
  await fresh();
  const scene = new THREE.Scene();
  const body = cuboid('temporary-cube');
  scene.add(body);
  try {
    scene.updateMatrixWorld(true);
    assert.deepEqual((await OpenGeometry.settled({ deflection: 0.05 })).failed, []);
    assert.equal(body.appearance.deflection, undefined);
    assert.equal(body.record?.bucket, 2 ** -5);
    await frame(scene, ortho(1.5 * P));
    assert.equal(body.record?.bucket, P);
  } finally {
    body.dispose();
  }
});

test('settled returns the captured OGError', async () => {
  await fresh();
  const scene = new THREE.Scene();
  const body = cuboid('failing-cube');
  scene.add(body);
  const failure = new OGError('InvalidGeometry', 'tessellation', 'stubbed tessellation failure');
  const provider = Object(TESTING.runtime().provider);
  const errors = listen('error');
  provider.compute = () => { throw failure; };
  try {
    scene.updateMatrixWorld(true);
    const { failed } = await OpenGeometry.settled();
    assert.equal(failed.length, 1);
    assert.equal(failed[0]?.error, failure);
  } finally {
    delete provider.compute;
    errors.stop();
    body.dispose();
  }
});

test('hysteresis keeps the bucket inside 0.75p and 4p and leaves it outside', async () => {
  await fresh();
  TIMERS.enable({ apis: ['setTimeout', 'Date'] });
  const scene = new THREE.Scene();
  const body = cuboid('hysteresis-cube');
  scene.add(body);
  const camera = ortho(1.5 * P);
  try {
    await frame(scene, camera);
    assert.equal(body.record?.bucket, P);
    const steps = [
      { target: 3.99999 * P, bucket: P }, { target: 4.00001 * P, bucket: 4 * P },
      { target: 3.00003 * P, bucket: 4 * P }, { target: 2.99997 * P, bucket: 2 * P },
    ];
    for (const { target, bucket } of steps) {
      await frame(scene, aim(camera, target));
      TIMERS.tick(151);
      assert.equal(body.record?.bucket, bucket, `target ${String(target / P)}p`);
    }
  } finally {
    TIMERS.reset();
    body.dispose();
  }
});

test('moving never coarsens and the settle timer refines 150 ms after the last change', async () => {
  await fresh();
  TIMERS.enable({ apis: ['setTimeout', 'Date'] });
  const scene = new THREE.Scene();
  const body = cuboid('moving-cube');
  scene.add(body);
  const camera = ortho(1.5 * P);
  let log;
  try {
    await frame(scene, camera);
    assert.equal(body.record?.bucket, P);
    log = computeLog();
    const buckets = () => log?.requests.map((request) => request.bucket);
    TIMERS.tick(100);
    await frame(scene, aim(camera, 6 * P));
    TIMERS.tick(100);
    await frame(scene, aim(camera, 0.19 * P));
    assert.deepEqual(buckets(), []);
    TIMERS.tick(149);
    assert.deepEqual(buckets(), []);
    TIMERS.tick(1);
    assert.deepEqual(buckets(), [P / 8]);
    assert.equal(body.record?.bucket, P / 8);
  } finally {
    log?.stop();
    TIMERS.reset();
    body.dispose();
  }
});

test('near and far instances share the finest bucket in both add orders', async () => {
  const half = Math.sqrt(3) / 2;
  const expected = 2 ** Math.floor(Math.log2(0.5 * 2 * Math.tan(Math.PI / 6) / 1000));
  for (const nearFirst of [true, false]) {
    await fresh();
    const near = cuboid('near-cube');
    const far = near.instance({ ogId: 'far-cube' });
    near.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, -0.5, -(1 + half)] });
    far.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, -0.5, -(100 + half)] });
    OpenGeometry.flush();
    const scene = new THREE.Scene();
    scene.add(...(nearFirst ? [near, far] : [far, near]));
    const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 1000);
    camera.updateMatrixWorld();
    try {
      await frame(scene, camera);
      assert.equal(expected, 2 ** -11);
      assert.equal(near.record?.bucket, expected, nearFirst ? 'near first' : 'far first');
      assert.equal(far.record?.bucket, expected, nearFirst ? 'near first' : 'far first');
    } finally {
      far.dispose();
      near.dispose();
    }
  }
});

test('a failed bucket is skipped and the coarser record is installed', async () => {
  await fresh();
  const body = cuboid('failed-bucket-cube');
  const warnings = listen('warning');
  const errors = listen('error');
  const log = computeLog();
  try {
    const { floor } = kernelBuckets(body);
    pinWantedBucket(body.lastInfo.shapeId, floor / 2);
    TESTING.ensureGeometry(body, true);
    assert.deepEqual(errors.events, []);
    assert.deepEqual(warnings.events, [{
      code: 'LimitExceeded', shapeId: body.lastInfo.shapeId, revision: body.lastInfo.shapeRevision,
      bucket: floor / 2, retryBucket: floor,
    }]);
    assert.equal(body.record?.bucket, floor);
    assert.equal(TESTING.wanted(body), floor);
    assert.equal(TESTING.wanted(body), floor);
    TESTING.ensureGeometry(body, true);
    assert.deepEqual(log.requests.map((request) => request.bucket), [floor / 2, floor]);
  } finally {
    log.stop();
    warnings.stop();
    errors.stop();
    body.dispose();
  }
});

test('the finest per-body override wins for one shape', async () => {
  for (const finerFirst of [true, false]) {
    await fresh();
    const first = cuboid('override-cube');
    const second = first.instance({ ogId: 'override-instance' });
    const [fine, coarse] = finerFirst ? [first, second] : [second, first];
    fine.setAppearance({ deflection: 0.01 });
    coarse.setAppearance({ deflection: 0.1 });
    const scene = new THREE.Scene();
    scene.add(first, second);
    try {
      scene.updateMatrixWorld(true);
      assert.equal(TESTING.wanted(first), 2 ** -7);
      assert.equal(TESTING.wanted(second), 2 ** -7);
    } finally {
      second.dispose();
      first.dispose();
    }
  }
});

test("the unit cube's floor and static bucket are the kernel's", async () => {
  await fresh();
  const aligned = cuboid('aligned-cube');
  const rotated = cuboid('rotated-cube');
  try {
    rotated.transform(OG_TRANSFORM_ROTATE, { axis: [0, 1, 0], degrees: 45, pivot: [0, 0, 0] });
    OpenGeometry.flush();
    aligned.setAppearance({ deflection: 1e-12 });
    assert.equal(kernelBuckets(aligned).floor, 2 ** -21);
    assert.equal(TESTING.wanted(aligned), 2 ** -21);
    assert.equal(kernelBuckets(rotated).static, 2 ** -9);
    assert.equal(TESTING.wanted(rotated), 2 ** -9);
  } finally {
    rotated.dispose();
    aligned.dispose();
  }
});

test('overlapping settled calls leave no temporary target behind', async () => {
  await fresh();
  const scene = new THREE.Scene();
  const body = cuboid('overlapping-cube');
  scene.add(body);
  try {
    scene.updateMatrixWorld(true);
    assert.deepEqual((await OpenGeometry.settled()).failed, []);
    const before = TESTING.wanted(body);
    assert.equal(before, 2 ** -9);
    const first = OpenGeometry.settled({ deflection: 2 ** -6 });
    const second = await OpenGeometry.settled({ deflection: 2 ** -8 });
    await first;
    assert.deepEqual(second.failed, []);
    assert.deepEqual((await OpenGeometry.settled()).failed, []);
    assert.equal(TESTING.wanted(body), before);
  } finally {
    body.dispose();
  }
});
