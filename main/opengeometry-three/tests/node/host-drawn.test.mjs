import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, OG_PRIMITIVE_CUBOID } from '../../../dist/index.js';
import { runtime } from '../../../dist/testing.js';
import { boot, cuboid, failsWith, nextTask } from './support.mjs';

async function fresh() {
  await boot();
  OpenGeometry.reset();
}

function current(body) {
  return body.record?.shapeId === body.lastInfo.shapeId && body.record?.revision === body.lastInfo.shapeRevision;
}

function stamped(body) {
  const state = runtime();
  return (state.displayed.get(body) ?? -Infinity) >= state.pass - 1;
}

function renderList(scene) {
  const listed = [];
  scene.traverseVisible((object) => { listed.push(object); });
  return listed;
}

function asyncProvider() {
  const provider = Object(runtime().provider);
  const compute = provider.compute.bind(provider);
  provider.compute = undefined;
  provider.request = (request) => new Promise((resolve) => { setTimeout(() => { resolve(compute(request)); }, 1); });
  return () => { delete provider.compute; delete provider.request; };
}

async function until(check) {
  for (let index = 0; index < 10 && !check(); index++) await nextTask();
  return check();
}

test('a host-drawn body and everything under it are left out of the render-list walk', async () => {
  await fresh();
  const [hosted, kept] = [cuboid('host-walk'), cuboid('host-kept', 2)];
  const scene = new THREE.Scene();
  scene.add(hosted, kept);
  try {
    OpenGeometry.setHostDrawn([hosted], true);
    assert.equal(hosted.visible, false);
    const listed = renderList(scene);
    assert.equal(listed.includes(hosted.surface) || listed.includes(hosted.outline), false);
    assert.equal(listed.includes(kept.surface), true);
    await nextTask();
  } finally {
    OpenGeometry.reset();
  }
});

test('markDisplayed stamps a host-drawn body and it gets its worker geometry', async () => {
  await fresh();
  const restore = asyncProvider();
  const body = cuboid('host-marked');
  try {
    OpenGeometry.setHostDrawn([body], true);
    OpenGeometry.markDisplayed([body]);
    assert.equal(stamped(body), true);
    assert.equal(await until(() => current(body)), true);
  } finally {
    restore();
    OpenGeometry.reset();
  }
});

test('a host-drawn body swaps to its new record after an edit', async () => {
  await fresh();
  const restore = asyncProvider();
  const body = cuboid('host-swap');
  try {
    OpenGeometry.setHostDrawn([body], true);
    OpenGeometry.markDisplayed([body]);
    assert.equal(await until(() => current(body)), true);
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    OpenGeometry.flush();
    OpenGeometry.markDisplayed([body]);
    assert.equal(await until(() => current(body)), true);
    assert.equal(body.visible, false);
  } finally {
    restore();
    OpenGeometry.reset();
  }
});

test('a sync flush of given bodies and a walk mesh a host-drawn body', async () => {
  await fresh();
  const [flushed, walked] = [cuboid('host-flushed'), cuboid('host-walked', 2)];
  try {
    OpenGeometry.setHostDrawn([flushed, walked], true);
    OpenGeometry.flush({ geometry: 'sync', bodies: [flushed] });
    assert.equal(current(flushed), true);
    walked.updateMatrixWorld(true);
    assert.equal(current(walked), true);
    await nextTask();
  } finally {
    OpenGeometry.reset();
  }
});

test('hide and revive while host-drawn keep the logical state and leave three.js visible false', async () => {
  await fresh();
  await nextTask();
  const body = cuboid('host-revived');
  const scene = new THREE.Scene();
  scene.add(body);
  OpenGeometry.setHostDrawn([body], true);
  const mark = OpenGeometry.mark();
  body.dispose();
  assert.equal(body.visible, false);
  OpenGeometry.markDisplayed([body]);
  assert.equal(runtime().displayed.has(body), false, 'a hidden host-drawn body is not stamped');
  mark.rollback();
  mark.release();
  await nextTask();
  assert.equal(body.parent, scene);
  assert.equal(body.visible, false);
  OpenGeometry.markDisplayed([body]);
  assert.equal(stamped(body), true, 'a revived host-drawn body is stamped');
  OpenGeometry.setHostDrawn([body], false);
  assert.equal(body.visible, true);
  await nextTask();
});

test('leaving the host draw restores three.js visible to the logical value exactly', async () => {
  await fresh();
  const [shown, hidden] = [cuboid('host-shown'), cuboid('host-hidden', 2)];
  hidden.visible = false;
  try {
    OpenGeometry.setHostDrawn([shown, hidden], true);
    OpenGeometry.setHostDrawn([shown, hidden], true);
    assert.deepEqual([shown.visible, hidden.visible], [false, false]);
    OpenGeometry.markDisplayed([hidden]);
    assert.equal(runtime().displayed.has(hidden), false);
    OpenGeometry.setHostDrawn([shown, hidden], false);
    OpenGeometry.setHostDrawn([shown, hidden], false);
    assert.deepEqual([shown.visible, hidden.visible], [true, false]);
    await nextTask();
  } finally {
    OpenGeometry.reset();
  }
});

test('reset while host-drawn leaves three.js visible false', async () => {
  await fresh();
  const body = cuboid('host-reset');
  OpenGeometry.setHostDrawn([body], true);
  OpenGeometry.reset();
  assert.equal(body.visible, false);
});

test('a body never handed to a host keeps its own visible through hide and revive', async () => {
  await fresh();
  await nextTask();
  const body = cuboid('never-hosted');
  const scene = new THREE.Scene();
  scene.add(body);
  const mark = OpenGeometry.mark();
  body.dispose();
  assert.equal(body.visible, false);
  mark.rollback();
  mark.release();
  await nextTask();
  assert.equal(body.visible, true);
  OpenGeometry.markDisplayed([body]);
  assert.equal(stamped(body), true);
  await nextTask();
});

test('setHostDrawn validates bodies as markDisplayed does', async () => {
  await fresh();
  const [body, disposed] = [cuboid('host-valid'), cuboid('host-disposed', 2)];
  disposed.dispose();
  try {
    const invalid = failsWith('InvalidParameter');
    assert.throws(() => Reflect.apply(OpenGeometry.setHostDrawn, OpenGeometry, [body, true]), invalid);
    assert.throws(() => Reflect.apply(OpenGeometry.setHostDrawn, OpenGeometry, [[new THREE.Mesh()], true]), invalid);
    assert.throws(() => OpenGeometry.setHostDrawn([disposed], true), failsWith('Disposed'));
    assert.equal(body.visible, true);
  } finally {
    OpenGeometry.reset();
  }
});

test('setHostDrawn false ignores a disposed body and true still throws Disposed', async () => {
  await fresh();
  const [disposed, kept] = [cuboid('host-release-disposed'), cuboid('host-release-kept', 2)];
  try {
    OpenGeometry.setHostDrawn([disposed, kept], true);
    disposed.dispose();
    const visible = disposed.visible;
    OpenGeometry.setHostDrawn([disposed, kept], false);
    assert.equal(disposed.visible, visible);
    assert.equal(kept.visible, true);
    assert.throws(() => OpenGeometry.setHostDrawn([disposed], true), failsWith('Disposed'));
    assert.equal(disposed.visible, visible);
  } finally {
    OpenGeometry.reset();
  }
});

test('setHostDrawn false ignores a body the kernel finalized during a render pass', async () => {
  await fresh();
  const [drawn, finalized] = [cuboid('host-pass-drawn'), cuboid('host-pass-finalized', 2)];
  const kept = cuboid('host-pass-kept', 3);
  const scene = new THREE.Scene();
  scene.add(drawn, finalized, kept);
  try {
    OpenGeometry.setHostDrawn([finalized, kept], true);
    OpenGeometry.markDisplayed([drawn]);
    assert.equal(runtime().renderPassActive, true);
    finalized.dispose();
    assert.equal([...runtime().bodies.values()].includes(finalized), false);
    OpenGeometry.setHostDrawn([finalized, kept], false);
    assert.equal(finalized.visible, false);
    assert.equal(kept.visible, true);
    assert.throws(() => OpenGeometry.setHostDrawn([finalized], true), failsWith('Disposed'));
    await nextTask();
    assert.equal(finalized.parent, null);
  } finally {
    OpenGeometry.reset();
  }
});
