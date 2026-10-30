import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry, OGError, Solid, Wire, OG_OPERATION_EXTRUDE, OG_OPERATION_REVOLVE, OG_OPERATION_SHELL,
  OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_PROFILE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { boot, failsWith } from './support.mjs';

const D_PAN = {
  start: [0, 0],
  segments: [
    { to: [0.325, 0] },
    { to: [0.325, 0.35], arc: { center: [0.325, 0.175], counterClockwise: true } },
    { to: [0, 0.35] },
    { to: [0, 0] },
  ],
  closed: true,
};

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

function coverageGap(families) {
  return (error) => {
    assert(error instanceof OGError);
    assert.equal(error.code, 'CoverageGap');
    assert.deepEqual(error.details.families, families);
    return true;
  };
}

function cavityFaces(body) {
  const brep = body.getBrep();
  assert.equal(brep.solids.length, 1);
  assert.equal(brep.solids[0].cavity_shells.length, 1);
  const shell = brep.topology.shells.find((candidate) => candidate.id === brep.solids[0].cavity_shells[0]);
  assert(shell);
  return new Set(shell.faces);
}

test('a shelled cuboid maps each of its six faces to one outer and one inner face', async () => {
  await boot();
  const box = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'hollow-box' });
  box.operate(OG_OPERATION_SHELL, { thickness: 0.1 });
  const inner = cavityFaces(box);
  const report = box.getReport();
  assert(report);
  assert.deepEqual(report.handlers, []);
  const mappings = report.report.face_mappings;
  assert.equal(mappings.length, 6);
  assert.equal(new Set(mappings.flatMap((mapping) => mapping.result_faces)).size, 12);
  for (const mapping of mappings) {
    assert.equal(mapping.result_faces.length, 2);
    assert.equal(mapping.result_faces.filter((face) => inner.has(face)).length, 1);
  }
});

test('an arc-edged pan shells into one solid with one cavity and two cylindrical STEP surfaces', async () => {
  await boot();
  const outline = new Wire(OG_PRIMITIVE_PROFILE, D_PAN, { ogId: 'shell-pan-outline' });
  const pan = new Solid(OG_OPERATION_EXTRUDE, { profile: outline, distance: 0.1 }, { ogId: 'shell-pan' });
  pan.operate(OG_OPERATION_SHELL, { thickness: 0.01 });
  assert.equal(cavityFaces(pan).size, 6);
  const { text } = await OpenGeometry.exportStep({ nodes: [pan] });
  assert.equal(text.match(/CYLINDRICAL_SURFACE/g)?.length, 2);
  assert.equal(text.match(/BREP_WITH_VOIDS/g)?.length, 1);
});

test('a bad thickness is InvalidParameter and one past the half width is UnresolvedIntersection', async () => {
  await boot();
  const box = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'solid-box' });
  for (const thickness of [Number.NaN, Number.POSITIVE_INFINITY, 0, -0.1]) {
    assert.throws(() => box.operate(OG_OPERATION_SHELL, { thickness }), failsWith('InvalidParameter'), `${thickness}`);
  }
  assert.throws(() => box.operate(OG_OPERATION_SHELL, { thickness: 0.6 }), failsWith('UnresolvedIntersection'));
  assert.deepEqual(box.getBrep().solids.map((solid) => solid.cavity_shells.length), [0]);
  assert.equal(box.getReport(), null);
});

test('a full-turn revolve shell is a CoverageGap that names the revolved profile family', async () => {
  await boot();
  const outline = new Wire(OG_PRIMITIVE_PROFILE, WC_BOWL, {
    ogId: 'shell-bowl-outline',
    plane: { normal: [0, 0, 1], xDirection: [1, 0, 0] },
  });
  const bowl = new Solid(OG_OPERATION_REVOLVE, { profile: outline }, { ogId: 'shell-bowl' });
  assert.throws(
    () => bowl.operate(OG_OPERATION_SHELL, { thickness: 0.005 }),
    coverageGap(['restricted analytic shell', 'revolved profile']),
  );
  assert.deepEqual(bowl.getBrep().solids.map((solid) => solid.cavity_shells.length), [0]);
});

test('a notched arc-edged extrusion shell keeps the arc-edged profile extrusion family', async () => {
  await boot();
  const outline = new Wire(OG_PRIMITIVE_PROFILE, D_PAN, { ogId: 'notched-pan-outline' });
  const pan = new Solid(OG_OPERATION_EXTRUDE, { profile: outline, distance: 0.1 }, { ogId: 'notched-pan' });
  const notch = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.1, height: 0.1, depth: 0.1 }, { ogId: 'pan-notch' });
  notch.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 0.05, 0] });
  pan.operate(OG_OPERATION_SUBTRACT, { tools: [notch] });
  assert.throws(
    () => pan.operate(OG_OPERATION_SHELL, { thickness: 0.005 }),
    coverageGap(['restricted analytic shell', 'arc-edged profile extrusion']),
  );
});
