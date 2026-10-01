import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, OGError, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE } from '../../../dist/index.js';
import * as TESTING from '../../../dist/testing.js';

const MODULE = new WebAssembly.Module(readFileSync(new URL('../../../dist/opengeometry_bg.wasm', import.meta.url)));

function runtime() {
  return (Reflect.get(TESTING, 'runtime') ?? Reflect.get(OpenGeometry, 'runtime'))();
}

function flushCount() {
  const read = Reflect.get(TESTING, 'flushCount');
  return read ? read() : Reflect.get(OpenGeometry, 'flushCount');
}

function boot() {
  return OpenGeometry.create({ wasmModule: MODULE });
}

function nextTask() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function cuboid(ogId, width = 1) {
  return new Solid(OG_PRIMITIVE_CUBOID, { width, height: 1, depth: 1 }, { ogId });
}

function failsWith(code) {
  return (error) => error instanceof OGError && error.code === code;
}

function thrownBy(block) {
  try {
    block();
    return undefined;
  } catch (error) {
    return error;
  }
}

test('a create that cannot load the kernel rejects and leaves no runtime', async () => {
  const error = await OpenGeometry.create().then(() => undefined, (reason) => reason);
  assert(error instanceof Error);
  assert.throws(() => runtime(), failsWith('NotInitialised'));
});

test('two concurrent create calls share one runtime', async () => {
  const instantiate = WebAssembly.instantiate;
  let instantiations = 0;
  Reflect.set(WebAssembly, 'instantiate', (...args) => {
    instantiations++;
    return Reflect.apply(instantiate, WebAssembly, args);
  });
  try {
    const [first, second] = await Promise.all([boot().then(() => runtime()), boot().then(() => runtime())]);
    assert.equal(first, second);
    assert.equal(instantiations, 1);
  } finally {
    Reflect.set(WebAssembly, 'instantiate', instantiate);
  }
});

test('instance and duplicate wrap the new node without the __existing kind', async () => {
  await boot();
  const rail = cuboid('rail');
  const instance = rail.instance({ ogId: 'rail-instance' });
  const copy = rail.duplicate({ ogId: 'rail-copy' });
  assert.equal(runtime().bodies.get('rail-instance'), instance);
  assert.equal(runtime().bodies.get('rail-copy'), copy);
  const wall = cuboid('wall', 2);
  const count = runtime().bodies.size;
  assert.throws(() => new Solid('__existing', {}, { ogId: 'wall' }), (error) => error instanceof OGError);
  assert.equal(runtime().bodies.get('wall'), wall);
  assert.equal(runtime().bodies.size, count);
});

test("clone places the display copy at the body's world matrix", async () => {
  await boot();
  const body = cuboid('clone-source');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 2, 3] });
  OpenGeometry.flush();
  const display = body.clone();
  const scene = new THREE.Scene();
  scene.add(display);
  scene.updateMatrixWorld(true);
  assert.deepEqual(new THREE.Vector3().setFromMatrixPosition(display.matrixWorld).toArray(), [1, 2, 3]);
});

test('cloning a display clone throws InvalidOperand', async () => {
  await boot();
  const display = cuboid('clone-twice').clone();
  assert.throws(() => display.clone(), failsWith('InvalidOperand'));
});

test('rollback in the same tick as a deferred removal keeps the revived body in its parent', async () => {
  await boot();
  await nextTask();
  const body = cuboid('revived', 3);
  const scene = new THREE.Scene();
  scene.add(body);
  const mark = OpenGeometry.mark();
  scene.updateMatrixWorld(true);
  body.dispose();
  mark.rollback();
  mark.release();
  await nextTask();
  assert.equal(body.parent, scene);
  assert.equal(body.visible, true);
});

test('disposing a sibling from a geometry listener during a render traversal defers its removal', async () => {
  await boot();
  await nextTask();
  const scene = new THREE.Scene();
  const first = cuboid('first-sibling', 4);
  const sibling = cuboid('second-sibling', 5);
  scene.add(first);
  scene.add(sibling);
  const stop = OpenGeometry.on('geometry', () => { if (sibling.visible) sibling.dispose(); });
  try {
    assert.equal(thrownBy(() => { scene.updateMatrixWorld(true); }), undefined);
    assert.equal(sibling.visible, false);
    await nextTask();
    assert.equal(sibling.parent, null);
  } finally {
    stop();
  }
});

test('reset disposes every body material', async () => {
  await boot();
  const body = cuboid('material-owner', 6);
  const disposed = [];
  Object(body.surface.material).addEventListener('dispose', () => { disposed.push('surface'); });
  Object(body.outline.material).addEventListener('dispose', () => { disposed.push('outline'); });
  OpenGeometry.reset();
  assert.deepEqual(disposed.sort(), ['outline', 'surface']);
});

test('a body from a reset runtime throws Disposed instead of validating against the new graph', async () => {
  OpenGeometry.reset();
  await boot();
  const stale = cuboid('stale-body');
  OpenGeometry.reset();
  await boot();
  cuboid('fresh-body');
  assert.throws(() => stale.getBounds(), failsWith('Disposed'));
});

test('reset leaves a usable empty document', async () => {
  await boot();
  const before = runtime();
  OpenGeometry.reset();
  assert.notEqual(runtime(), before);
  assert.equal(runtime().bodies.size, 0);
  const body = cuboid('after-reset');
  assert.equal(runtime().bodies.get('after-reset'), body);
});

test('a Solid inside a Solid flushes once on updateMatrixWorld', async () => {
  await boot();
  await nextTask();
  const outer = cuboid('outer', 7);
  const inner = cuboid('inner', 8);
  const scene = new THREE.Scene();
  outer.add(inner);
  scene.add(outer);
  const before = flushCount();
  const caught = thrownBy(() => { scene.updateMatrixWorld(true); });
  const flushes = flushCount() - before;
  outer.remove(inner);
  assert.equal(caught, undefined);
  assert.equal(flushes, 1);
});
