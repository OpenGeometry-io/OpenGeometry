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
  OG_PRIMITIVE_CONE,
  OG_PRIMITIVE_CUBOID,
  OG_PRIMITIVE_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_CONE,
  OG_PRIMITIVE_PARAMS_CYLINDER_SECTOR,
  OG_PRIMITIVE_PARAMS_POLYHEDRON,
  OG_PRIMITIVE_PARAMS_SPHERE,
  OG_PRIMITIVE_PARAMS_TORUS,
  OG_PRIMITIVE_POLYHEDRON,
  OG_PRIMITIVE_PROFILE,
  OG_PRIMITIVE_RECTANGLE,
  OG_PRIMITIVE_SPHERE,
  OG_PRIMITIVE_TORUS,
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

async function assertOneSurface(body, surface) {
  assert.equal(body.lastInfo.bodyType, 'Solid');
  const { text } = await OpenGeometry.exportStep({ nodes: [body] });
  assert.equal(text.match(surface)?.length, 1);
}

test('a sphere, a cone and a torus are Solids that export one analytic STEP surface each', async () => {
  await boot();
  const knob = new Solid(OG_PRIMITIVE_SPHERE, OG_PRIMITIVE_PARAMS_SPHERE({ radius: 0.5 }), { ogId: 'knob' });
  await assertOneSurface(knob, /SPHERICAL_SURFACE/g);
  const cone = OG_PRIMITIVE_PARAMS_CONE({ radiusBottom: 1, radiusTop: 0.4, height: 2 });
  await assertOneSurface(new Solid(OG_PRIMITIVE_CONE, cone, { ogId: 'pedestal' }), /CONICAL_SURFACE/g);
  const torus = OG_PRIMITIVE_PARAMS_TORUS({ majorRadius: 1, minorRadius: 0.25 });
  await assertOneSurface(new Solid(OG_PRIMITIVE_TORUS, torus, { ogId: 'torus-ring' }), /TOROIDAL_SURFACE/g);
});

test('a torus with minor radius equal to major and a cone with two zero radii are InvalidParameter', async () => {
  await boot();
  const torus = { majorRadius: 1, minorRadius: 1 };
  assert.throws(() => new Solid(OG_PRIMITIVE_TORUS, torus, { ogId: 'spindle' }), failsWith('InvalidParameter'));
  const cone = { radiusBottom: 0, radiusTop: 0, height: 1 };
  assert.throws(() => new Solid(OG_PRIMITIVE_CONE, cone, { ogId: 'needle' }), failsWith('InvalidParameter'));
});

function dShape(inset) {
  const far = 0.35 - inset;
  return {
    start: [inset, inset],
    segments: [
      { to: [0.325, inset] },
      { to: [0.325, far], arc: { center: [0.325, 0.175], counterClockwise: true } },
      { to: [inset, far] },
      { to: [inset, inset] },
    ],
    closed: true,
  };
}

test('an arc-edged pan minus a blind inset is one Solid built by the prismatic curved handler', async () => {
  await boot();
  const outline = new Wire(OG_PRIMITIVE_PROFILE, dShape(0), { ogId: 'pan-outline' });
  const pan = new Solid(OG_OPERATION_EXTRUDE, { profile: outline, distance: 0.4 }, { ogId: 'pan' });
  const rim = new Wire(OG_PRIMITIVE_PROFILE, dShape(0.03), { ogId: 'inset-outline', plane: { origin: [0, 0.05, 0] } });
  const inset = new Solid(OG_OPERATION_EXTRUDE, { profile: rim, distance: 0.4 }, { ogId: 'inset' });
  pan.operate(OG_OPERATION_SUBTRACT, { tools: [inset] });
  assert.deepEqual(pan.getReport()?.handlers, ['prismatic_curved_boolean']);
  assert.equal(pan.getBrep().solids.length, 1);
  const { text } = await OpenGeometry.exportStep({ nodes: [pan] });
  assert.equal(text.match(/CYLINDRICAL_SURFACE/g)?.length, 2);
});
