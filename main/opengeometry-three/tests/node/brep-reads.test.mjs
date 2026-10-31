import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry, Sheet, Solid, Wire, OG_OPERATION_SWEEP, OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_POLYLINE,
  OG_PRIMITIVE_RECTANGLE, OG_TRANSFORM_ROTATE, OG_TRANSFORM_SCALE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { graph } from '../../../dist/testing.js';
import { boot, cuboid, failsWith } from './support.mjs';

function assertNear(actual, expected, tolerance) {
  assert.equal(actual.length, expected.length);
  actual.forEach((value, index) => {
    assert(Math.abs(value - (expected[index] ?? Number.NaN)) <= tolerance, `${String(value)} at ${String(index)}`);
  });
}

function applied(matrix, [x, y, z]) {
  return [0, 1, 2].map((row) => matrix[row] * x + matrix[row + 4] * y + matrix[row + 8] * z + matrix[row + 12]);
}

function combined(frame, [a, b, c], origin = frame.origin) {
  const at = (index) => Number(origin[index] + frame.x[index] * a + frame.y[index] * b + frame.z[index] * c);
  return [at(0), at(1), at(2)];
}

function surfaces(body) {
  return JSON.parse(graph().brep(body.ogId)).geometry.surfaces;
}

test("getWorldBrep of a moved cuboid holds its vertices moved by the kernel's world matrix", async () => {
  await boot();
  const body = cuboid('world-moved');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [3, -2, 7] });
  body.transform(OG_TRANSFORM_ROTATE, { axis: [0.2, 1, 0.4], degrees: 35 });
  const matrix = Array.from(graph().worldMatrix(body.ogId));
  const local = body.getBrep();
  const world = body.getWorldBrep();
  assert.equal(world.id, local.id);
  assert.equal(world.revision, local.revision);
  assertNear(
    world.topology.vertices.flatMap((vertex) => vertex.position),
    local.topology.vertices.flatMap((vertex) => applied(matrix, vertex.position)),
    1e-12,
  );
  body.dispose();
});

test('getWorldBrep of an unmoved body is the getBrep string', async () => {
  await boot();
  const body = cuboid('world-still');
  assert.equal(graph().worldBrep(body.ogId), graph().brep(body.ogId));
  assert.deepEqual(body.getWorldBrep(), body.getBrep());
  body.dispose();
});

test('getWorldBrep of a scaled body scales vertex tolerance', async () => {
  await boot();
  const body = cuboid('world-scaled');
  body.transform(OG_TRANSFORM_SCALE, { factor: 4 });
  const local = body.getBrep();
  const world = body.getWorldBrep();
  assert.deepEqual(
    world.topology.vertices.map((vertex) => vertex.tolerance),
    local.topology.vertices.map((vertex) => vertex.tolerance * 4),
  );
  assert.equal(world.accuracy.geometric, local.accuracy.geometric * 4);
  body.dispose();
});

test('faceNormalAt gives each cuboid face its outward normal and a cylinder side its radial normal', async () => {
  await boot();
  const body = cuboid('normal-box');
  const brep = body.getBrep();
  const positions = brep.topology.vertices.map((vertex) => vertex.position);
  const centre = [0, 1, 2].map(
    (axis) => positions.reduce((sum, position) => sum + (position[axis] ?? Number.NaN), 0) / 8,
  );
  const boxSurfaces = surfaces(body);
  const normals = brep.topology.faces.map((face) => {
    const { origin } = boxSurfaces[face.surface].frame;
    const normal = body.faceNormalAt(face.id, origin);
    assert(normal.reduce((sum, value, axis) => sum + value * (origin[axis] - (centre[axis] ?? Number.NaN)), 0) > 0);
    return normal.join();
  });
  assert.equal(new Set(normals).size, 6);
  const cylinder = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.5, height: 2 }, { ogId: 'normal-cylinder' });
  const sideSurfaces = surfaces(cylinder);
  const side = cylinder.getBrep().topology.faces.find((face) => sideSurfaces[face.surface].kind === 'Cylinder');
  assert(side);
  const { frame } = sideSurfaces[side.surface];
  const [x = 0, y = 0, z = 0] = combined(frame, [0.5 * Math.cos(0.7), 0.5 * Math.sin(0.7), 0.3]);
  const radial = combined(frame, [Math.cos(0.7), Math.sin(0.7), 0], [0, 0, 0]);
  assertNear(cylinder.faceNormalAt(side.id, [x, y, z]), radial, 1e-12);
  body.dispose();
  cylinder.dispose();
});

test('faceNormalAt rejects -1, 1.5, 2**32 and a two-number point with InvalidParameter', async () => {
  await boot();
  const body = cuboid('normal-reject');
  for (const faceId of [-1, 1.5, 2 ** 32]) {
    assert.throws(() => body.faceNormalAt(faceId, [0, 0, 0]), failsWith('InvalidParameter'));
  }
  assert.throws(() => Reflect.apply(body.faceNormalAt, body, [0, [0, 0]]), failsWith('InvalidParameter'));
  assert.throws(() => body.faceNormalAt(6, [0, 0, 0]), failsWith('InvalidParameter'));
  body.dispose();
});

test('faceNormalAt checks the ogId before the point, so an unknown ogId with a bad point is UnknownNode', async () => {
  await boot();
  assert.throws(
    () => graph().faceNormalAt('normal-missing', 0, new Float64Array(2)),
    (error) => JSON.parse(String(error)).code === 'UnknownNode',
  );
});

test("a disposed body's reads throw Disposed", async () => {
  await boot();
  const body = cuboid('reads-disposed');
  body.dispose();
  assert.throws(() => body.getWorldBrep(), failsWith('Disposed'));
  assert.throws(() => body.faceNormalAt(0, [0, 0, 0]), failsWith('Disposed'));
});

test("classifyPoint answers in the body's own coordinates, and a Wire is InvalidOperand", async () => {
  await boot();
  const body = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.5, height: 2 }, { ogId: 'classify-cylinder' });
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [10, 0, 0] });
  assert.equal(body.classifyPoint([0, 1, 0]), 'Inside');
  assert.equal(body.classifyPoint([10, 1, 0]), 'Outside');
  assert.equal(body.classifyPoint([0.1, 2, 0.1]), 'Boundary');
  assert.throws(() => Reflect.apply(body.classifyPoint, body, [[0, 0]]), failsWith('InvalidParameter'));
  const wire = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId: 'classify-wire' });
  assert.throws(() => wire.classifyPoint([0, 0, 0]), failsWith('InvalidOperand'));
  body.dispose();
  wire.dispose();
});

test('classifyPoint refuses a Sheet with InvalidOperand and a NaN coordinate with InvalidParameter', async () => {
  await boot();
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId: 'classify-sheet-profile' });
  const points = [[0, 0, 0], [0, 3, 0]];
  const path = new Wire(OG_PRIMITIVE_POLYLINE, { points }, { ogId: 'classify-sheet-path' });
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile, path, caps: false }, { ogId: 'classify-sheet' });
  assert.throws(() => sheet.classifyPoint([0, 1, 0]), failsWith('InvalidOperand'));
  const body = cuboid('classify-nan');
  assert.throws(() => body.classifyPoint([Number.NaN, 0, 0]), failsWith('InvalidParameter'));
  body.dispose();
  sheet.dispose();
  path.dispose();
  profile.dispose();
});

function state(body, revision) {
  const changes = graph().changesSince(revision).changesJson;
  return JSON.stringify([String(graph().revision()), changes, OpenGeometry.markStats(), graph().brep(body.ogId)]);
}

test('both reads leave revision, changes, mark stats and getBrep unchanged', async () => {
  await boot();
  const body = cuboid('reads-state');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 2, 3] });
  const revision = graph().revision();
  const before = state(body, revision);
  body.getWorldBrep();
  body.faceNormalAt(0, [0, 0, 0]);
  assert.throws(() => body.faceNormalAt(99, [0, 0, 0]), failsWith('InvalidParameter'));
  assert.equal(state(body, revision), before);
  body.dispose();
});

test('measure bounds a cylinder against its exact volume and a cuboid measures exactly', async () => {
  await boot();
  const cylinder = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.5, height: 2 }, { ogId: 'measure-cylinder' });
  const coarse = cylinder.measure();
  assert.equal(coarse.deflection, 0.01);
  assert(Math.abs(coarse.volume - Math.PI * 0.5) <= coarse.errorBound);
  const fine = cylinder.measure({ deflection: 0.001 });
  assert.equal(fine.deflection, 0.001);
  assert(fine.errorBound < coarse.errorBound);
  assert(Math.abs(fine.volume - Math.PI * 0.5) <= fine.errorBound);
  const body = cuboid('measure-box', 2);
  const box = body.measure();
  assert(Math.abs(box.volume - 2) <= 1e-12);
  assert(Math.abs(box.area - 10) <= 1e-12);
  assert(box.errorBound <= 1e-8);
  cylinder.dispose();
  body.dispose();
});

test('areaErrorBound bounds the area of a millimetre cylinder and of a cuboid within 1e-8', async () => {
  await boot();
  const cylinder = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.001, height: 0.002 }, { ogId: 'measure-area' });
  const size = cylinder.measure({ deflection: 0.00001 });
  assert(Math.abs(size.area - Math.PI * 0.000006) <= size.areaErrorBound);
  assert(size.areaErrorBound < size.area * 0.1);
  const body = cuboid('measure-area-box', 2);
  const box = body.measure();
  assert(Math.abs(box.area - 10) <= box.areaErrorBound);
  assert(box.areaErrorBound <= 1e-8);
  cylinder.dispose();
  body.dispose();
});

test('measure refuses a deflection that is not a positive number with InvalidParameter', async () => {
  await boot();
  const body = cuboid('measure-reject');
  for (const deflection of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(() => body.measure({ deflection }), failsWith('InvalidParameter'));
  }
  assert.throws(() => Reflect.apply(body.measure, body, [{ deflection: '0.1' }]), failsWith('InvalidParameter'));
  body.dispose();
  assert.throws(() => body.measure(), failsWith('Disposed'));
  assert.throws(() => body.validity(), failsWith('Disposed'));
});

test('validity reports a closed valid Solid, an open Sheet and closed and open Wires', async () => {
  await boot();
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId: 'validity-profile' });
  const points = [[0, 0, 0], [0, 3, 0]];
  const path = new Wire(OG_PRIMITIVE_POLYLINE, { points }, { ogId: 'validity-path' });
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile, path, caps: false }, { ogId: 'validity-sheet' });
  const body = cuboid('validity-box');
  assert.deepEqual(body.validity(), { valid: true, closed: true, bodyType: 'Solid' });
  assert.deepEqual(sheet.validity(), { valid: true, closed: false, bodyType: 'Sheet' });
  assert.deepEqual(profile.validity(), { valid: true, closed: true, bodyType: 'Wire' });
  assert.deepEqual(path.validity(), { valid: true, closed: false, bodyType: 'Wire' });
  body.dispose();
  sheet.dispose();
  path.dispose();
  profile.dispose();
});

test('measure and validity leave revision, changes, mark stats and getBrep unchanged', async () => {
  await boot();
  const body = cuboid('measure-state');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 2, 3] });
  const revision = graph().revision();
  const before = state(body, revision);
  body.measure();
  body.measure({ deflection: 0.002 });
  body.validity();
  assert.throws(() => body.measure({ deflection: -1 }), failsWith('InvalidParameter'));
  assert.equal(state(body, revision), before);
  body.dispose();
});
