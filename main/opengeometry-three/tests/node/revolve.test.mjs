import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_PARAMS_REVOLVE, OG_OPERATION_REVOLVE, OG_PRIMITIVE_PROFILE,
  OG_SUPPORTS_PARTIAL_REVOLVE,
} from '../../../dist/index.js';
import { boot, failsWith } from './support.mjs';

const WC_BOWL = {
  start: [0, 0],
  segments: [
    { to: [0.12, 0] },
    { to: [0.19, 0.3] },
    { to: [0.19, 0.34], arc: { center: [0.19, 0.32], counterClockwise: true } },
    { to: [0.16, 0.34] },
    { to: [0, 0.18], arc: { center: [0, 0.34], counterClockwise: false } },
    { to: [0, 0] },
  ],
  closed: true,
};

function polygon(points) {
  return { start: points[0], segments: [...points.slice(1), points[0]].map((to) => ({ to })), closed: true };
}

function upright(params) {
  return new Wire(OG_PRIMITIVE_PROFILE, params, { plane: { normal: [0, 0, 1], xDirection: [1, 0, 0] } });
}

function bounds(body) {
  const box = body.getBounds();
  assert(box);
  return box;
}

function surfaces(text, kind) {
  return text.split(kind).length - 1;
}

test('a WC bowl profile revolves upright and exports one analytic surface per edge', async () => {
  await boot();
  const profile = upright(WC_BOWL);
  const bowl = new Solid(OG_OPERATION_REVOLVE, OG_OPERATION_PARAMS_REVOLVE({ profile }), { ogId: 'wc-bowl' });
  assert.equal(bowl.getBrep().topology.faces.length, 5);
  const [minX, minY, , maxX, maxY] = bounds(bowl);
  assert(minY > -1e-6 && maxY > 0.34 - 1e-6 && maxY < 0.37, `${minY} ${maxY}`);
  assert(minX < -0.2 && maxX > 0.2, `${minX} ${maxX}`);
  const { text, report } = await OpenGeometry.exportStep({ nodes: [bowl], unit: 'metre' });
  assert.equal(report.solids, 1);
  assert.equal(report.pcurvelessEdges, 0);
  for (const kind of ['SPHERICAL_SURFACE(', 'TOROIDAL_SURFACE(', 'CONICAL_SURFACE(']) {
    assert.equal(surfaces(text, kind), 1, kind);
  }
});

test('the default axis is the v axis of the default plane, which maps [u, v] to (u, 0, -v)', async () => {
  await boot();
  const profile = new Wire(OG_PRIMITIVE_PROFILE, polygon([[1, 0], [2, 0], [2, 3], [1, 3]]));
  const tube = new Solid(OG_OPERATION_REVOLVE, { profile }, { ogId: 'lying-tube' });
  const [minX, minY, minZ, maxX, maxY, maxZ] = bounds(tube);
  assert(minX < -1.9 && maxX > 1.9 && minY < -1.9 && maxY > 1.9, `${minX} ${maxX} ${minY} ${maxY}`);
  assert(minZ < -2.9 && maxZ > -0.1 && maxZ < 0.1, `${minZ} ${maxZ}`);
});

test('an axis in the plane revolves a rectangle into a tube, classified in body coordinates', async () => {
  await boot();
  const profile = upright(polygon([[0, 0], [1, 0], [1, 2], [0, 2]]));
  const axis = { point: [-1, 0], direction: [0, 1] };
  const tube = new Solid(OG_OPERATION_REVOLVE, { profile, axis }, { ogId: 'tube' });
  assert.equal(tube.classifyPoint([-1, 1.5, -1]), 'Inside');
  assert.equal(tube.classifyPoint([-1, 0, -1]), 'Outside');
});

test('a profile crossing the axis is refused as InvalidParameter', async () => {
  await boot();
  const profile = upright(polygon([[-1, 0], [1, 0], [1, 1], [-1, 1]]));
  assert.throws(() => new Solid(OG_OPERATION_REVOLVE, { profile }), failsWith('InvalidParameter'));
});

test('rebuild revolves the changed profile and keeps a solid', async () => {
  await boot();
  const profile = upright(polygon([[1, 0], [2, 0], [2, 1], [1, 1]]));
  const ring = new Solid(OG_OPERATION_REVOLVE, { profile }, { ogId: 'rebuilt-ring' });
  profile.rebuild(OG_PRIMITIVE_PROFILE, polygon([[1, 0], [3, 0], [3, 1], [1, 1]]));
  ring.rebuild(OG_OPERATION_REVOLVE, { profile });
  assert.equal(ring.lastInfo.bodyType, 'Solid');
  assert(bounds(ring)[3] > 2.9);
});

test('a revolve with holes is refused as InvalidParameter', async () => {
  await boot();
  const profile = upright(polygon([[1, 0], [3, 0], [3, 2], [1, 2]]));
  const hole = upright(polygon([[1.5, 0.5], [2.5, 0.5], [2.5, 1.5], [1.5, 1.5]]));
  assert.throws(() => new Solid(OG_OPERATION_REVOLVE, { profile, holes: [hole] }), failsWith('InvalidParameter'));
});

test('signed partial revolutions retain exact rounded sides, caps and STEP topology', async () => {
  await boot();
  assert.equal(OG_SUPPORTS_PARTIAL_REVOLVE, true);
  const profile = upright({
    start: [1.025, 0],
    segments: [
      { to: [1, 0] },
      { to: [0.99, -0.01], arc: { center: [1, -0.01], counterClockwise: true } },
      { to: [0.99, -0.03] },
      { to: [1, -0.04], arc: { center: [1, -0.03], counterClockwise: true } },
      { to: [1.025, -0.04] },
      { to: [1.025, 0] },
    ],
    closed: true,
  });
  for (const degrees of [90, 270, 360, -90, -270, -360]) {
    const angle = degrees * Math.PI / 180;
    const part = new Solid(
      OG_OPERATION_REVOLVE, OG_OPERATION_PARAMS_REVOLVE({ profile, angle }), { ogId: `round-${degrees}` },
    );
    const brep = part.getBrep();
    assert.equal(brep.topology.faces.length, Math.abs(degrees) === 360 ? 6 : 8);
    const { text, report } = await OpenGeometry.exportStep({ nodes: [part], unit: 'metre' });
    assert.equal(report.solids, 1);
    assert.equal(report.pcurvelessEdges, 0);
    assert.equal(surfaces(text, 'TOROIDAL_SURFACE('), 2);
    assert.equal(part.lastInfo.bodyType, 'Solid');
  }
});

test('partial revolve sign chooses the intended side and rejected edits preserve the old solid', async () => {
  await boot();
  const profile = upright(polygon([[1, 0], [2, 0], [2, 1], [1, 1]]));
  const quarter = new Solid(OG_OPERATION_REVOLVE, { profile, angle: Math.PI / 2 });
  assert.equal(quarter.classifyPoint([1, -1, -0.5]), 'Inside');
  assert.equal(quarter.classifyPoint([1, 1, -0.5]), 'Outside');
  const negative = new Solid(OG_OPERATION_REVOLVE, { profile, angle: -Math.PI / 2 });
  assert.equal(negative.classifyPoint([1, 1, -0.5]), 'Inside');
  assert.equal(negative.classifyPoint([1, -1, -0.5]), 'Outside');
  const old = quarter.getBrep();
  for (const angle of [0, 2 * Math.PI + 0.1, -2 * Math.PI - 0.1, 1e-12, 2 * Math.PI - 1e-12]) {
    assert.throws(() => quarter.rebuild(OG_OPERATION_REVOLVE, { profile, angle }), failsWith('InvalidParameter'));
    assert.deepEqual(quarter.getBrep(), old);
  }
});
