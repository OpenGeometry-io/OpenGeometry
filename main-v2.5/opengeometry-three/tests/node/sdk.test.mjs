import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_ROTATE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';

const BYTES = readFileSync(new URL('../../../dist/opengeometry_bg.wasm', import.meta.url));
await OpenGeometry.create({ wasmModule: new WebAssembly.Module(BYTES) });
assert.equal(OpenGeometry.activeBackend, 'inline');
const BODY = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'node-cube' });
const EXPORTED = await OpenGeometry.exportStep({ nodes: [BODY] });
assert.equal(EXPORTED.report.products, 1);
OpenGeometry.noteDisplayed(BODY);
assert.equal((await OpenGeometry.settled()).failed.length, 0);
BODY.dispose();
assert.equal((await OpenGeometry.settled()).failed.length, 0);
const SIZED = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'size-cache' });
const GET_BOUNDS = SIZED.getBounds.bind(SIZED);
let boundsReads = 0;
SIZED.getBounds = () => { boundsReads++; return GET_BOUNDS(); };
for (let index = 0; index < 100; index++) OpenGeometry.wanted(SIZED);
assert.equal(boundsReads, 1);
const CAMERA = new THREE.PerspectiveCamera(55, 1, 0.1, 100);
CAMERA.position.set(3, 3, 3);
CAMERA.lookAt(0, 0, 0);
CAMERA.updateMatrixWorld();
const RENDERER = { getDrawingBufferSize: (target) => target.set(800, 600) };
for (let index = 0; index < 100; index++) {
  Reflect.apply(SIZED.surface.onBeforeRender, SIZED.surface, [RENDERER, null, CAMERA]);
}
assert.equal(boundsReads, 2);
const MATERIAL = SIZED.surface.material;
SIZED.setAppearance({ deflection: 0.01 });
assert.equal(SIZED.surface.material, MATERIAL);
SIZED.setAppearance({ color: 0x22c55e });
assert.notEqual(SIZED.surface.material, MATERIAL);
for (let index = 0; index < 100; index++) {
  Reflect.apply(SIZED.surface.onBeforeRender, SIZED.surface, [RENDERER, null, CAMERA]);
}
assert.equal(boundsReads, 2);
SIZED.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
OpenGeometry.flush();
OpenGeometry.wanted(SIZED);
assert.equal(boundsReads, 2);
SIZED.transform(OG_TRANSFORM_ROTATE, { axis: [0, 1, 0], degrees: 30, pivot: [0, 0, 0] });
OpenGeometry.flush();
OpenGeometry.wanted(SIZED);
assert.equal(boundsReads, 3);
SIZED.dispose();
const VISIBLE = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 1 }, { ogId: 'visible-cube' });
OpenGeometry.noteDisplayed(VISIBLE);
const PROVIDER = Object(OpenGeometry.runtime().provider);
const COMPUTE = PROVIDER.compute.bind(PROVIDER);
PROVIDER.compute = undefined;
PROVIDER.request = async (request) => {
  await new Promise((resolve) => setTimeout(resolve, 2));
  return COMPUTE(request);
};
const PREPARED = new Solid(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 }, { ogId: 'prepared-cube' });
OpenGeometry.ensureGeometry(PREPARED);
assert(VISIBLE.record);
assert.equal(PREPARED.record, undefined);
assert.equal((await OpenGeometry.settled()).failed.length, 0);
assert(PREPARED.record);
assert(VISIBLE.record);
let cancelled = 0;
PROVIDER.cancelShape = () => { cancelled++; };
const RAPID = new Solid(OG_PRIMITIVE_CUBOID, { width: 2.5, height: 1, depth: 1 }, { ogId: 'rapid-cube' });
OpenGeometry.ensureGeometry(RAPID);
OpenGeometry.ensureGeometry(RAPID, true);
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
  OpenGeometry.ensureGeometry(replacement);
  assert(replacement.record);
  replacement.dispose();
  assert.equal((await OpenGeometry.settled()).failed.length, 0);
}
OpenGeometry.reset();
console.log('Node SDK passed');
