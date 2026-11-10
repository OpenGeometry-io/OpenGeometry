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
  OG_PRIMITIVE_PARAMS_POLYSOLID,
  OG_PRIMITIVE_PARAMS_SPHERE,
  OG_PRIMITIVE_PARAMS_TORUS,
  OG_PRIMITIVE_POLYHEDRON,
  OG_PRIMITIVE_POLYSOLID,
  OG_PRIMITIVE_PROFILE,
  OG_PRIMITIVE_RECTANGLE,
  OG_PRIMITIVE_SPHERE,
  OG_PRIMITIVE_TORUS,
  OG_TRANSFORM_TRANSLATE,
  OG_PRIMITIVE_SLOT_PRISM,
  OG_PRIMITIVE_ROUNDED_BOX,
  OG_PRIMITIVE_PIPE_SEGMENT,
  OG_PRIMITIVE_PARAMS_SLOT_PRISM,
  OG_PRIMITIVE_PARAMS_ROUNDED_BOX,
  OG_PRIMITIVE_PARAMS_PIPE_SEGMENT,
  OG_PRIMITIVE_REGULAR_POLYGON,
  OG_PRIMITIVE_SLOT,
  OG_PRIMITIVE_ROUNDED_RECTANGLE,
  OG_PRIMITIVE_PARAMS_REGULAR_POLYGON,
  OG_PRIMITIVE_PARAMS_SLOT,
  OG_PRIMITIVE_PARAMS_ROUNDED_RECTANGLE,
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

const STANDARD_SOLIDS = [
  {
    kind: OG_PRIMITIVE_SLOT_PRISM,
    params: OG_PRIMITIVE_PARAMS_SLOT_PRISM({ length: 1, width: 0.5, height: 1 }),
    cylinders: 2,
    volume: 0.5 + Math.PI / 16,
  },
  {
    kind: OG_PRIMITIVE_ROUNDED_BOX,
    params: OG_PRIMITIVE_PARAMS_ROUNDED_BOX({ width: 1, height: 1, depth: 1, radius: 0.1 }),
    cylinders: 4,
    volume: 1 - (4 - Math.PI) * 0.01,
  },
  {
    kind: OG_PRIMITIVE_PIPE_SEGMENT,
    params: OG_PRIMITIVE_PARAMS_PIPE_SEGMENT({
      outerRadius: 0.5, innerRadius: 0.3, height: 1, startAngle: 0, sweepAngle: Math.PI / 2,
    }),
    cylinders: 2,
    volume: Math.PI * 0.16 / 4,
  },
];

test('slot prism, rounded box and pipe segment Solids have exact volumes and cylindrical faces', async () => {
  await boot();
  for (const { kind, params, cylinders, volume } of STANDARD_SOLIDS) {
    const solid = new Solid(kind, params, { ogId: kind });
    assert.equal(solid.lastInfo.bodyType, 'Solid');
    const measured = solid.measure({ deflection: 1e-4 });
    assert.equal(Math.abs(measured.volume - volume) <= measured.errorBound, true, kind);
    const { text } = await OpenGeometry.exportStep({ nodes: [solid] });
    assert.equal(text.match(/CYLINDRICAL_SURFACE/g)?.length, cylinders, kind);
  }
});

test('a rounded box radius of half its width and an inverted pipe segment are InvalidParameter', async () => {
  await boot();
  const corner = { width: 1, height: 1, depth: 2, radius: 0.5 };
  assert.throws(() => new Solid(OG_PRIMITIVE_ROUNDED_BOX, corner, { ogId: 'corner' }), failsWith('InvalidParameter'));
  const pipe = { outerRadius: 0.3, innerRadius: 0.5, height: 1, startAngle: 0, sweepAngle: 1 };
  assert.throws(() => new Solid(OG_PRIMITIVE_PIPE_SEGMENT, pipe, { ogId: 'pipe' }), failsWith('InvalidParameter'));
});

function lPath(arc) {
  return { start: [0, 0], segments: [{ to: [3, 0] }, { to: [3, 2], arc }], closed: false };
}

test('an L-shaped polysolid is a Solid of volume one that exports planes only to STEP', async () => {
  await boot();
  const params = OG_PRIMITIVE_PARAMS_POLYSOLID({
    path: { start: [0, 0], segments: [{ to: [3, 0] }, { to: [3, 2] }], closed: false },
    width: 0.2,
    height: 1,
    justify: 'centre',
  });
  const wall = new Solid(OG_PRIMITIVE_POLYSOLID, params, { ogId: 'l-wall' });
  assert.equal(wall.lastInfo.bodyType, 'Solid');
  assert.equal(Math.abs(wall.measure().volume - 1) <= 1e-9, true);
  const { text } = await OpenGeometry.exportStep({ nodes: [wall] });
  assert.equal(text.match(/\bPLANE\(/g)?.length, wall.getBrep().topology.faces.length);
  assert.equal(text.match(/CYLINDRICAL_SURFACE/g), null);
});

test('a polysolid with a tangent arc is a curved Solid and a sharp arc corner is InvalidParameter', async () => {
  await boot();
  const params = { path: lPath({ center: [3, 1], counterClockwise: true }), width: 0.2, height: 1 };
  const wall = new Solid(OG_PRIMITIVE_POLYSOLID, params, { ogId: 'curved' });
  assert.equal(Math.abs(wall.measure({ deflection: 1e-4 }).volume - (0.6 + 0.2 * Math.PI)) <= 1e-3, true);
  const { text } = await OpenGeometry.exportStep({ nodes: [wall] });
  assert.equal(text.match(/CYLINDRICAL_SURFACE/g)?.length, 2);
  const sharp = { ...params, path: lPath({ center: [3.5, 1], counterClockwise: true }) };
  assert.throws(() => new Solid(OG_PRIMITIVE_POLYSOLID, sharp, { ogId: 'sharp' }), failsWith('InvalidParameter'));
});

test('regular polygon, slot and rounded rectangle Wires extrude into prisms with their edges', async () => {
  await boot();
  const wires = [
    {
      kind: OG_PRIMITIVE_REGULAR_POLYGON,
      params: OG_PRIMITIVE_PARAMS_REGULAR_POLYGON({ sides: 6, radius: 0.5 }),
      edges: 6,
    },
    { kind: OG_PRIMITIVE_SLOT, params: OG_PRIMITIVE_PARAMS_SLOT({ length: 1, width: 0.5 }), edges: 4 },
    {
      kind: OG_PRIMITIVE_ROUNDED_RECTANGLE,
      params: OG_PRIMITIVE_PARAMS_ROUNDED_RECTANGLE({ width: 1.2, breadth: 0.8, radius: 0.1 }),
      edges: 8,
    },
  ];
  for (const { kind, params, edges } of wires) {
    const profile = new Wire(kind, params, { ogId: kind });
    assert.equal(profile.lastInfo.bodyType, 'Wire');
    assert.equal(profile.getBrep().topology.edges.length, edges, kind);
    const solid = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: 1 }, { ogId: `${kind}-solid` });
    assert.equal(solid.getBrep().topology.faces.length, edges + 2, kind);
  }
  for (const sides of [2, 5.5, -3]) {
    const params = { sides, radius: 1 };
    const options = { ogId: `sides-${sides}` };
    assert.throws(() => new Wire(OG_PRIMITIVE_REGULAR_POLYGON, params, options), failsWith('InvalidParameter'));
  }
});
