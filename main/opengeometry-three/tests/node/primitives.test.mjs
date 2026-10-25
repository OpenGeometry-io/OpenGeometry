import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry,
  Solid,
  Wire,
  OG_OPERATION_EXTRUDE,
  OG_OPERATION_SUBTRACT,
  OG_PRIMITIVE_ARC,
  OG_PRIMITIVE_CIRCLE,
  OG_PRIMITIVE_CUBOID,
  OG_PRIMITIVE_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_POLYHEDRON,
  OG_PRIMITIVE_POLYHEDRON,
  OG_PRIMITIVE_PROFILE,
  OG_PRIMITIVE_RECTANGLE,
  OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { boot, failsWith } from './support.mjs';

const RING_SECTOR = {
  start: [1, 0],
  segments: [
    { to: [2, 0] },
    { to: [0, 2], arc: { center: [0, 0], counterClockwise: true } },
    { to: [0, 1] },
    { to: [1, 0], arc: { center: [0, 0], counterClockwise: false } },
  ],
  closed: true,
};

test('Profile and Arc create Wires', async () => {
  await boot();
  assert.equal(new Wire(OG_PRIMITIVE_PROFILE, RING_SECTOR, { ogId: 'ring' }).lastInfo.bodyType, 'Wire');
  const arc = new Wire(OG_PRIMITIVE_ARC, { radius: 2, startAngle: 0, sweepAngle: Math.PI / 2 }, { ogId: 'arc' });
  assert.equal(arc.lastInfo.bodyType, 'Wire');
});

test('an extruded ring sector exports cylindrical STEP faces', async () => {
  await boot();
  const profile = new Wire(OG_PRIMITIVE_PROFILE, RING_SECTOR, { ogId: 'sector' });
  const solid = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: 3 }, { ogId: 'sector-solid' });
  const { text } = await OpenGeometry.exportStep({ nodes: [solid] });
  assert.equal(text.match(/CYLINDRICAL_SURFACE/g)?.length, 2);
});

test('an unknown segment field is InvalidParameter', async () => {
  await boot();
  const params = { start: [0, 0], segments: [{ to: [1, 0], bulge: 1 }, { to: [0, 1] }, { to: [0, 0] }], closed: true };
  assert.throws(() => new Wire(OG_PRIMITIVE_PROFILE, params, { ogId: 'unknown' }), failsWith('InvalidParameter'));
});

test('a hole that encloses the profile is InvalidParameter', async () => {
  await boot();
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 4, breadth: 4 }, { ogId: 'square' });
  const hole = new Wire(OG_PRIMITIVE_CIRCLE, { radius: 5 }, { ogId: 'enclosing' });
  assert.throws(
    () => new Solid(OG_OPERATION_EXTRUDE, { profile, holes: [hole], distance: 1 }, { ogId: 'enclosed' }),
    failsWith('InvalidParameter'),
  );
});

test('a cylinder sector is a Solid that exports one cylindrical STEP face', async () => {
  await boot();
  const params = OG_PRIMITIVE_PARAMS_CYLINDER_SECTOR({ radius: 2, height: 3, startAngle: 0, sweepAngle: Math.PI / 2 });
  const sector = new Solid(OG_PRIMITIVE_CYLINDER_SECTOR, params, { ogId: 'cylinder-sector' });
  assert.equal(sector.lastInfo.bodyType, 'Solid');
  const { text } = await OpenGeometry.exportStep({ nodes: [sector] });
  assert.equal(text.match(/CYLINDRICAL_SURFACE/g)?.length, 1);
});

test('a full-turn cylinder sector is InvalidParameter', async () => {
  await boot();
  const params = { radius: 2, height: 3, startAngle: 0, sweepAngle: 2 * Math.PI };
  assert.throws(() => new Solid(OG_PRIMITIVE_CYLINDER_SECTOR, params, { ogId: 'full' }), failsWith('InvalidParameter'));
});

const HIP_FACES = [[0, 1, 2, 3], [1, 0, 4, 5], [2, 1, 5], [3, 2, 5, 4], [0, 3, 4]];

function hipRoof(ridgeY = 1) {
  return OG_PRIMITIVE_PARAMS_POLYHEDRON({
    points: [[0, 0, 0], [4, 0, 0], [4, 0, 2], [0, 0, 2], [1, 1, 1], [3, ridgeY, 1]],
    faces: HIP_FACES,
  });
}

test('a hip roof polyhedron is a five-face Solid that exports planes to STEP', async () => {
  await boot();
  const roof = new Solid(OG_PRIMITIVE_POLYHEDRON, hipRoof(), { ogId: 'hip-roof' });
  assert.equal(roof.getBrep().topology.faces.length, 5);
  const { text } = await OpenGeometry.exportStep({ nodes: [roof] });
  assert.equal(text.match(/\bPLANE\(/g)?.length, 5);
});

test('a non-planar face is InvalidParameter and inward rings are InvalidTopology', async () => {
  await boot();
  const bent = hipRoof(1.001);
  assert.throws(() => new Solid(OG_PRIMITIVE_POLYHEDRON, bent, { ogId: 'bent' }), failsWith('InvalidParameter'));
  const inward = { ...hipRoof(), faces: HIP_FACES.map((ring) => [...ring].reverse()) };
  assert.throws(() => new Solid(OG_PRIMITIVE_POLYHEDRON, inward, { ogId: 'inward' }), failsWith('InvalidTopology'));
});

test('a roof minus a chimney goes through the planar polyhedra handler', async () => {
  await boot();
  const roof = new Solid(OG_PRIMITIVE_POLYHEDRON, hipRoof(), { ogId: 'chimney-roof' });
  const chimney = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.5, height: 3, depth: 0.5 }, { ogId: 'chimney' });
  chimney.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, -1, 1] });
  roof.operate(OG_OPERATION_SUBTRACT, { tools: [chimney] });
  assert.deepEqual(roof.getReport()?.handlers, ['subtract_planar_polyhedra']);
});
