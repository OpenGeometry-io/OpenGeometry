import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry,
  Solid,
  Wire,
  OG_OPERATION_EXTRUDE,
  OG_PRIMITIVE_ARC,
  OG_PRIMITIVE_CIRCLE,
  OG_PRIMITIVE_PROFILE,
  OG_PRIMITIVE_RECTANGLE,
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
