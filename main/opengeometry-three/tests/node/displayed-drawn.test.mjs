import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE } from '../../../dist/index.js';
import { graph, runtime } from '../../../dist/testing.js';
import { boot, cuboid, failsWith, listen, nextTask } from './support.mjs';

const RENDERER = { info: { render: { frame: 0 } }, getDrawingBufferSize: (target) => target.set(1000, 1000) };
const CAMERA = new THREE.PerspectiveCamera(50, 1, 0.1, 1000);
const BODY = Object.getPrototypeOf(Solid.prototype);

async function fresh() {
  await boot();
  OpenGeometry.reset();
  CAMERA.position.set(0, 0, 10);
  CAMERA.updateMatrixWorld();
}

function draw(bodies) {
  RENDERER.info.render.frame++;
  for (const body of bodies) Reflect.apply(body.surface.onBeforeRender, body.surface, [RENDERER, null, CAMERA]);
}

async function drawFrame(bodies) {
  draw(bodies);
  await nextTask();
}

function displayed(body) {
  const state = runtime();
  return body.visible && (state.displayed.get(body) ?? -Infinity) >= state.pass - 1;
}

function current(body) {
  return body.record?.shapeId === body.lastInfo.shapeId && body.record?.revision === body.lastInfo.shapeRevision;
}

function installsOn(body, step) {
  const install = BODY.install;
  let calls = 0;
  BODY.install = function counted(...rest) {
    if (this === body) calls++;
    return Reflect.apply(install, this, rest);
  };
  try {
    step();
  } finally {
    BODY.install = install;
  }
  return calls;
}

function asyncProvider() {
  const provider = Object(runtime().provider);
  const compute = provider.compute.bind(provider);
  provider.compute = undefined;
  provider.request = (request) => new Promise((resolve) => { setTimeout(() => { resolve(compute(request)); }, 1); });
  return () => { delete provider.compute; delete provider.request; };
}

test('a body drawn once and then not drawn for two passes is not displayed and gets no swap', async () => {
  await fresh();
  const [gone, kept] = [cuboid('drawn-gone'), cuboid('drawn-kept', 2)];
  try {
    await drawFrame([gone, kept]);
    assert.equal(displayed(gone), true, 'a drawn body is displayed');
    assert.equal(current(gone), true, 'a drawn body gets its geometry');
    await drawFrame([kept]);
    await drawFrame([kept]);
    assert.equal(displayed(gone), false);
    const installs = installsOn(gone, () => {
      gone.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
      OpenGeometry.flush();
    });
    assert.equal(installs, 0);
    assert.equal(current(gone), false);
  } finally {
    OpenGeometry.reset();
  }
});

test('a body stamped only through markDisplayed on layer 30 gets its worker geometry', async () => {
  await fresh();
  const restore = asyncProvider();
  const body = cuboid('marked-only');
  body.layers.set(30);
  try {
    OpenGeometry.markDisplayed([body]);
    assert.equal(body.record, undefined, 'no install inside the draw');
    assert.equal(displayed(body), true);
    for (let index = 0; index < 5 && !current(body); index++) await nextTask();
    assert.equal(current(body), true);
  } finally {
    restore();
    OpenGeometry.reset();
  }
});

test('markDisplayed rejects what flush({ bodies }) rejects and skips a hidden body', async () => {
  await fresh();
  const hidden = cuboid('marked-hidden');
  const disposed = cuboid('marked-disposed');
  hidden.visible = false;
  disposed.dispose();
  try {
    const invalid = failsWith('InvalidParameter');
    assert.throws(() => Reflect.apply(OpenGeometry.markDisplayed, OpenGeometry, [hidden]), invalid);
    assert.throws(() => Reflect.apply(OpenGeometry.markDisplayed, OpenGeometry, [[new THREE.Mesh()]]), invalid);
    assert.throws(() => OpenGeometry.markDisplayed([disposed]), failsWith('Disposed'));
    OpenGeometry.markDisplayed([hidden]);
    assert.equal(runtime().displayed.has(hidden), false);
    await nextTask();
  } finally {
    OpenGeometry.reset();
  }
});

test('a draw-stamped body installs at pass end, a walk-stamped body at once', async () => {
  await fresh();
  const [drawn, walked] = [cuboid('order-drawn'), cuboid('order-walked', 2)];
  try {
    const geometry = drawn.surface.geometry;
    draw([drawn]);
    assert.equal(drawn.record, undefined, 'no install inside the draw hook');
    assert.equal(drawn.surface.geometry, geometry);
    walked.updateMatrixWorld(true);
    assert.equal(current(walked), true, 'a walk in the same pass installs at once');
    await Promise.resolve();
    assert.equal(current(drawn), true, 'the drawn body installs when the pass ends');
    drawn.rebuild(OG_PRIMITIVE_CUBOID, { width: 4, height: 1, depth: 1 });
    walked.updateMatrixWorld(true);
    draw([drawn]);
    assert.equal(current(drawn), false, 'a walk earlier in the pass does not install a later draw stamp');
    await Promise.resolve();
    assert.equal(current(drawn), true);
  } finally {
    OpenGeometry.reset();
  }
});

async function walkedAndDrawn(prefix) {
  const bodies = Array.from({ length: 50 }, (_, index) => cuboid(`${prefix}-${String(index)}`, 1 + index / 100));
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  await drawFrame(bodies);
  return { scene, bodies };
}

function counted(target, name) {
  const original = target[name];
  const log = { calls: 0, stop: () => { Reflect.deleteProperty(target, name); } };
  target[name] = (...args) => { log.calls++; return Reflect.apply(original, target, args); };
  return log;
}

async function realFrame(scene, bodies) {
  runtime().lod.lastSchedule = Date.now();
  scene.updateMatrixWorld(true);
  draw(bodies);
  await Promise.resolve();
}

test('a walk and a draw of 50 bodies in one pass, pass end included, read the graph revision once', async () => {
  await fresh();
  const { scene, bodies } = await walkedAndDrawn('reads');
  const log = counted(Object(graph()), 'revision');
  try {
    await realFrame(scene, bodies);
  } finally {
    log.stop();
    OpenGeometry.reset();
  }
  assert.equal(log.calls, 1, `${String(log.calls)} revision reads in one pass over ${String(bodies.length)} bodies`);
});

test('a walk and a draw of 50 bodies in one pass look up each record once', async () => {
  await fresh();
  const { scene, bodies } = await walkedAndDrawn('gets');
  const log = counted(runtime().records, 'get');
  try {
    await realFrame(scene, bodies);
  } finally {
    log.stop();
    OpenGeometry.reset();
  }
  assert.equal(log.calls, bodies.length);
});

test('a throwing geometrychanged listener is reported and the next pass still advances', async () => {
  await fresh();
  const errors = listen('error');
  const source = cuboid('wedge-source');
  try {
    await drawFrame([source]);
    const copy = source.instance({ ogId: 'wedge-copy' });
    OpenGeometry.flush();
    const failure = new Error('host listener');
    copy.addEventListener('geometrychanged', () => { throw failure; });
    await drawFrame([source, copy]);
    const pass = runtime().pass;
    const later = cuboid('wedge-later', 3);
    await drawFrame([source, later]);
    assert.equal(runtime().renderPassActive, false);
    assert.equal(runtime().pass, pass + 1);
    assert.equal(current(later), true);
    assert.deepEqual(errors.events, [failure]);
  } finally {
    errors.stop();
    OpenGeometry.reset();
  }
});

test('a throwing placementchanged listener leaves the other moved bodies placed', async () => {
  await fresh();
  const errors = listen('error');
  const [thrower, other] = [cuboid('placed-thrower'), cuboid('placed-other', 2)];
  const failure = new Error('placement listener');
  thrower.addEventListener('placementchanged', () => { throw failure; });
  try {
    thrower.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    other.transform(OG_TRANSFORM_TRANSLATE, { offset: [5, 0, 0] });
    OpenGeometry.flush();
    assert.equal(thrower.matrixWorld.elements[12], 1);
    assert.equal(other.matrixWorld.elements[12], 5);
    assert.deepEqual(errors.events, [failure]);
    OpenGeometry.flush();
    assert.equal(errors.events.length, 1);
  } finally {
    errors.stop();
    OpenGeometry.reset();
  }
});

test('geometrychanged fires once per install after the geometry is set', async () => {
  await fresh();
  const body = cuboid('event-geometry');
  const seen = [];
  body.addEventListener('geometrychanged', () => { seen.push(body.surface.geometry === body.record?.surface); });
  try {
    body.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    body.updateMatrixWorld(true);
    await nextTask();
    assert.deepEqual(seen, [true]);
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 1 });
    body.updateMatrixWorld(true);
    await nextTask();
    assert.deepEqual(seen, [true, true]);
  } finally {
    OpenGeometry.reset();
  }
});

test('placementchanged fires once per moved body per flush', async () => {
  await fresh();
  const [moved, edited] = [cuboid('event-moved'), cuboid('event-edited', 2)];
  const counts = { moved: 0, edited: 0 };
  moved.addEventListener('placementchanged', () => { counts.moved++; });
  edited.addEventListener('placementchanged', () => { counts.edited++; });
  try {
    moved.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    edited.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    OpenGeometry.flush();
    OpenGeometry.flush();
    assert.deepEqual(counts, { moved: 1, edited: 0 });
  } finally {
    OpenGeometry.reset();
  }
});

test('a host that walks its bodies and then syncs gets meshes for every walked body', async () => {
  await fresh();
  const bodies = Array.from({ length: 5 }, (_, index) => cuboid(`walk-sync-${String(index)}`, 1 + index / 10));
  const scene = new THREE.Scene();
  scene.add(...bodies);
  try {
    scene.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    assert.deepEqual(bodies.map(current), bodies.map(() => true));
    assert(bodies.every((body) => (body.record?.triangles ?? 0) > 0));
    await nextTask();
  } finally {
    OpenGeometry.reset();
  }
});
