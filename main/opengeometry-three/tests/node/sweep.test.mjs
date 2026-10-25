import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_SWEEP, OG_PRIMITIVE_ARC, OG_PRIMITIVE_CIRCLE, OG_PRIMITIVE_RECTANGLE,
} from '../../../dist/index.js';
import { boot } from './support.mjs';

function elbow(ogId, profile, radius) {
  const path = new Wire(OG_PRIMITIVE_ARC, { radius, startAngle: 0, sweepAngle: Math.PI / 2 });
  return new Solid(OG_OPERATION_SWEEP, { profile, path }, { ogId });
}

function atArcStart(kind, params, radius) {
  return new Wire(kind, params, { plane: { origin: [radius, 0, 0], normal: [0, 0, -1], xDirection: [1, 0, 0] } });
}

function surfaces(text, kind) {
  return text.split(kind).length - 1;
}

test('the default duct elbow sweeps along one arc and exports to STEP', async () => {
  await boot();
  const profile = atArcStart(OG_PRIMITIVE_RECTANGLE, { width: 0.4, breadth: 0.25 }, 0.6);
  const duct = elbow('duct-elbow', profile, 0.6);
  assert.equal(duct.getBrep().topology.faces.length, 6);
  const { text, report } = await OpenGeometry.exportStep({ nodes: [duct], unit: 'metre' });
  assert.equal(report.pcurvelessEdges, 0);
  assert.equal(surfaces(text, 'CYLINDRICAL_SURFACE('), 2);
});

test('a round pipe elbow gives torus faces', async () => {
  await boot();
  const profile = atArcStart(OG_PRIMITIVE_CIRCLE, { radius: 0.1 }, 0.3);
  const pipe = elbow('pipe-elbow', profile, 0.3);
  assert.equal(pipe.getBrep().topology.faces.length, 4);
  const { text, report } = await OpenGeometry.exportStep({ nodes: [pipe], unit: 'metre' });
  assert.equal(report.pcurvelessEdges, 0);
  assert.equal(surfaces(text, 'TOROIDAL_SURFACE('), 2);
});
