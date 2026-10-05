import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, OGError, Solid, Wire, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_PARAMS_RECTANGLE,
  OG_PRIMITIVE_RECTANGLE, OG_TRANSFORM_ROTATE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { activeBackend, ensureGeometry, graph, noteDisplayed, runtime, wanted } from '../../../dist/testing.js';

const BYTES = readFileSync(new URL('../../../dist/opengeometry_bg.wasm', import.meta.url));
await OpenGeometry.create({ wasmModule: new WebAssembly.Module(BYTES) });
assert.equal(activeBackend(), 'inline');
const BODY = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'node-cube' });
const EXPORTED = await OpenGeometry.exportStep({ nodes: [BODY] });
assert.equal(EXPORTED.report.products, 1);
noteDisplayed(BODY);
assert.equal((await OpenGeometry.settled()).failed.length, 0);
BODY.dispose();
assert.equal((await OpenGeometry.settled()).failed.length, 0);
const STYLED = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'styled-cube' });
const MATERIAL = STYLED.surface.material;
STYLED.setAppearance({ deflection: 0.01 });
assert.equal(STYLED.surface.material, MATERIAL);
STYLED.setAppearance({ color: 0x22c55e });
assert.notEqual(STYLED.surface.material, MATERIAL);
STYLED.dispose();
const VISIBLE = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 1 }, { ogId: 'visible-cube' });
noteDisplayed(VISIBLE);
const PROVIDER = Object(runtime().provider);
const COMPUTE = PROVIDER.compute.bind(PROVIDER);
PROVIDER.compute = undefined;
PROVIDER.request = async (request) => {
  await new Promise((resolve) => setTimeout(resolve, 2));
  return COMPUTE(request);
};
const PREPARED = new Solid(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 }, { ogId: 'prepared-cube' });
ensureGeometry(PREPARED);
assert(VISIBLE.record);
assert.equal(PREPARED.record, undefined);
assert.equal((await OpenGeometry.settled()).failed.length, 0);
assert(PREPARED.record);
assert(VISIBLE.record);
let cancelled = 0;
PROVIDER.cancelShape = () => { cancelled++; };
const RAPID = new Solid(OG_PRIMITIVE_CUBOID, { width: 2.5, height: 1, depth: 1 }, { ogId: 'rapid-cube' });
ensureGeometry(RAPID);
ensureGeometry(RAPID, true);
assert(RAPID.record);
assert.equal(cancelled, 1);
assert.equal((await OpenGeometry.settled()).failed.length, 0);
assert(RAPID.record);
RAPID.dispose();
PREPARED.dispose();
VISIBLE.dispose();
delete PROVIDER.compute;
delete PROVIDER.request;
delete PROVIDER.cancelShape;
assert.equal((await OpenGeometry.settled()).failed.length, 0);
for (let index = 0; index < 24; index++) {
  const size = { width: 1 + index / 10, height: 1, depth: 1 };
  const replacement = new Solid(OG_PRIMITIVE_CUBOID, size, { ogId: `replacement-${index}` });
  ensureGeometry(replacement);
  assert(replacement.record);
  replacement.dispose();
  assert.equal((await OpenGeometry.settled()).failed.length, 0);
}
const LEAK_PARAMS = OG_PRIMITIVE_PARAMS_RECTANGLE({ width: 1, breadth: 1 });
assert.throws(
  () => new Solid(OG_PRIMITIVE_RECTANGLE, LEAK_PARAMS, { ogId: 'leak' }),
  (error) => error instanceof OGError && error.code === 'BodyTypeMismatch',
);
new Wire(OG_PRIMITIVE_RECTANGLE, LEAK_PARAMS, { ogId: 'leak' });
assert.equal((await OpenGeometry.settled()).failed.length, 0);
OpenGeometry.reset();
console.log('Node SDK passed');

test('display buckets are read once per shape and revision', () => {
  const sized = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'size-cache' });
  const target = Object(graph());
  const read = target.displayBuckets;
  let reads = 0;
  target.displayBuckets = (...args) => { reads++; return Reflect.apply(read, target, args); };
  const camera = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
  camera.position.set(3, 3, 3);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld();
  const renderer = { getDrawingBufferSize: (size) => size.set(800, 600) };
  try {
    for (let index = 0; index < 100; index++) wanted(sized);
    assert.equal(reads, 1);
    for (let index = 0; index < 100; index++) {
      Reflect.apply(sized.surface.onBeforeRender, sized.surface, [renderer, null, camera]);
    }
    assert.equal(reads, 1);
    sized.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    OpenGeometry.flush();
    wanted(sized);
    assert.equal(reads, 1);
    sized.transform(OG_TRANSFORM_ROTATE, { axis: [0, 1, 0], degrees: 30, pivot: [0, 0, 0] });
    OpenGeometry.flush();
    wanted(sized);
    assert.equal(reads, 1);
    sized.rebuild(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 1 });
    wanted(sized);
    assert.equal(reads, 2);
  } finally {
    delete target.displayBuckets;
    sized.dispose();
  }
});
