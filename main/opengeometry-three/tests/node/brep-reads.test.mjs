import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry, Solid, OG_PRIMITIVE_CYLINDER, OG_TRANSFORM_ROTATE, OG_TRANSFORM_SCALE, OG_TRANSFORM_TRANSLATE,
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
