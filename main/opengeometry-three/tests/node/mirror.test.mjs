import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  Solid, Wire, OG_OPERATION_EXTRUDE, OG_PRIMITIVE_PROFILE, OG_TRANSFORM_PLACE, OG_TRANSFORM_SCALE,
} from '../../../dist/index.js';
import { boot, cuboid, failsWith } from './support.mjs';

const ORIGIN = [0.3, -0.2, 0.5];
const NORMAL = [1, 2, -0.5];

const ARM = {
  start: [0, 0],
  segments: [
    { to: [0.55, 0] },
    { to: [0.55, 0.08], arc: { center: [0.55, 0.04], counterClockwise: true } },
    { to: [0, 0.08] },
    { to: [0, 0] },
  ],
  closed: true,
};

function at(values, axis) {
  return values[axis] ?? Number.NaN;
}

function reflectedVector(vector) {
  const length = Math.hypot(...NORMAL);
  const unit = NORMAL.map((value) => value / length);
  const along = vector.reduce((sum, value, axis) => sum + value * at(unit, axis), 0);
  return vector.map((value, axis) => value - 2 * along * at(unit, axis));
}

function reflectedPoint(point) {
  const offset = point.map((value, axis) => value - at(ORIGIN, axis));
  return reflectedVector(offset).map((value, axis) => value + at(ORIGIN, axis));
}

function mirrorAcrossPlane(body, all = false) {
  body.mirror({ origin: [0.3, -0.2, 0.5], normal: [1, 2, -0.5] }, all ? { instances: 'all' } : {});
}

function faceSamples(body) {
  const brep = body.getBrep();
  return brep.topology.faces.map((face) => {
    const use = brep.topology.halfedges.find((halfedge) => halfedge.face === face.id);
    assert(use);
    const point = brep.topology.vertices[use.from].position;
    return { face: face.id, point, normal: body.faceNormalAt(face.id, point) };
  });
}

function chairArm(ogId) {
  const outline = new Wire(OG_PRIMITIVE_PROFILE, ARM, { ogId: `${ogId}-outline` });
  return new Solid(OG_OPERATION_EXTRUDE, { profile: outline, distance: 0.06 }, { ogId });
}

test('a mirrored chair arm reports the reflected normal on every face and keeps its placement', async () => {
  await boot();
  const arm = chairArm('mirror-arm');
  const samples = faceSamples(arm);
  const placement = arm.getPlacement();
  const volume = arm.measure().volume;
  mirrorAcrossPlane(arm);
  assert.deepEqual(arm.getPlacement(), placement);
  assert(Math.abs(arm.measure().volume - volume) < 1e-6);
  for (const { face, point, normal } of samples) {
    const actual = arm.faceNormalAt(face, reflectedPoint(point));
    const expected = reflectedVector(normal);
    actual.forEach((value, axis) => assert(Math.abs(value - expected[axis]) < 1e-9, `face ${face}`));
  }
});

test('a shared shape mirrors only with instances all and a zero normal is InvalidParameter', async () => {
  await boot();
  const box = cuboid('mirror-shared');
  const copy = box.instance();
  assert.throws(() => mirrorAcrossPlane(box), failsWith('SharedShape'));
  assert.throws(
    () => box.mirror({ origin: [0, 0, 0], normal: [0, 0, 0] }, { instances: 'all' }), failsWith('InvalidParameter'),
  );
  mirrorAcrossPlane(box, true);
  assert.deepEqual(copy.getBrep().topology.vertices, box.getBrep().topology.vertices);
});

test('a reflecting PLACE or SCALE still throws InvalidTransform', async () => {
  await boot();
  const box = cuboid('mirror-place');
  const placement = box.getPlacement();
  assert.throws(() => box.transform(OG_TRANSFORM_PLACE, { scale: -1 }), failsWith('InvalidTransform'));
  assert.throws(() => box.transform(OG_TRANSFORM_SCALE, { factor: -1 }), failsWith('InvalidTransform'));
  assert.deepEqual(box.getPlacement(), placement);
});
