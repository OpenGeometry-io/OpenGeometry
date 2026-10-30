import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Sheet, Solid, Wire, OG_OPERATION_SWEEP, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_POLYLINE,
  OG_PRIMITIVE_RECTANGLE,
} from '../../../dist/index.js';
import { boot, cuboid, failsWith } from './support.mjs';

function rejection(promise) {
  return promise.then(() => undefined, (reason) => reason);
}

function sweepInputs(prefix) {
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId: `${prefix}-profile` });
  const points = [[0, 0, 0], [0, 3, 0], [3, 3, 0]];
  const path = new Wire(OG_PRIMITIVE_POLYLINE, { points }, { ogId: `${prefix}-path` });
  return { profile, path, dispose: () => { profile.dispose(); path.dispose(); } };
}

function vertexCount(object) {
  return object.geometry.getAttribute('position')?.count ?? 0;
}

test('an open-ended sweep is a Sheet with an open shell and no solid', async () => {
  await boot();
  const inputs = sweepInputs('open');
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path, caps: false });
  const brep = sheet.getBrep();
  assert.equal(sheet.bodyType, 'Sheet');
  assert.equal(sheet.lastInfo.bodyType, 'Sheet');
  assert.equal(brep.topology.faces.length, 8);
  assert.equal(brep.topology.shells[0]?.is_closed, false);
  assert.deepEqual(brep.solids, []);
  assert(brep.topology.faces.every((face) => !face.key.startsWith('cap-')));
  sheet.dispose();
  inputs.dispose();
});

test('a Sheet displays its surface and its outline', async () => {
  await boot();
  const inputs = sweepInputs('shown');
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path, caps: false });
  const scene = new THREE.Scene();
  scene.add(sheet);
  try {
    scene.updateMatrixWorld(true);
    assert.deepEqual((await OpenGeometry.settled()).failed, []);
    assert.equal(sheet.surface.visible, true);
    assert.equal(sheet.outline.visible, true);
    assert(sheet.record?.surface);
    assert.equal(sheet.surface.geometry, sheet.record.surface);
    assert.equal(sheet.outline.geometry, sheet.record.outline);
    assert(vertexCount(sheet.surface) > 0);
    assert(vertexCount(sheet.outline) > 0);
  } finally {
    sheet.dispose();
    inputs.dispose();
  }
});

test('exportStep refuses a Sheet with InvalidOperand', async () => {
  await boot();
  const inputs = sweepInputs('export');
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path, caps: false });
  assert(failsWith('InvalidOperand')(await rejection(OpenGeometry.exportStep({ nodes: [sheet] }))));
  sheet.dispose();
  inputs.dispose();
});

test('a Solid sweep with caps false is BodyTypeMismatch and leaves no node', async () => {
  await boot();
  const inputs = sweepInputs('mismatch');
  const params = { profile: inputs.profile, path: inputs.path, caps: false };
  assert.throws(() => new Solid(OG_OPERATION_SWEEP, params, { ogId: 'mismatch-solid' }), failsWith('BodyTypeMismatch'));
  cuboid('mismatch-solid').dispose();
  inputs.dispose();
});

test('a Sheet of a primitive is BodyTypeMismatch and leaves no node', async () => {
  await boot();
  const params = { width: 1, height: 1, depth: 1 };
  const options = { ogId: 'mismatch-sheet' };
  assert.throws(() => new Sheet(OG_PRIMITIVE_CUBOID, params, options), failsWith('BodyTypeMismatch'));
  cuboid('mismatch-sheet').dispose();
});

test('a capped sweep is the same with caps true or left out', async () => {
  await boot();
  const inputs = sweepInputs('capped');
  const absent = new Solid(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path });
  const capped = new Solid(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path, caps: true });
  const strip = (brep) => JSON.stringify(brep).replaceAll(brep.id, '');
  assert.equal(strip(capped.getBrep()), strip(absent.getBrep()));
  assert.equal(absent.getBrep().topology.faces.length, 10);
  absent.dispose();
  capped.dispose();
  inputs.dispose();
});

test('a Sheet copies as a Sheet and rebuilds only as a Sheet', async () => {
  await boot();
  const inputs = sweepInputs('copies');
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path, caps: false });
  const copy = sheet.duplicate();
  const shared = sheet.instance();
  assert(copy instanceof Sheet && shared instanceof Sheet);
  assert.equal(copy.lastInfo.bodyType, 'Sheet');
  assert.throws(
    () => copy.rebuild(OG_OPERATION_SWEEP, { profile: inputs.profile, path: inputs.path }),
    failsWith('BodyTypeMismatch'),
  );
  shared.dispose();
  copy.dispose();
  sheet.dispose();
  inputs.dispose();
});
