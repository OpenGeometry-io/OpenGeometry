import assert from 'node:assert/strict';
import { test } from 'node:test';
import { OpenGeometry, OG_REGION_TOLERANCE } from '../../../dist/index.js';
import { boot, failsWith, listen } from './support.mjs';

await boot();

function wire(value) {
  return JSON.parse(JSON.stringify(value));
}

function clockwise(x0, z0, x1, z1, y = 0) {
  return [[x0, z0], [x0, z1], [x1, z1], [x1, z0]].map(([x, z]) => ({ x, y, z }));
}

function region(x0, z0, x1, z1, hole, y = 0) {
  return { outer: clockwise(x0, z0, x1, z1, y), holes: hole ? [clockwise(...hole, y).reverse()] : [] };
}

function signedArea(ring) {
  return ring.reduce((sum, point, index) => {
    const next = ring[(index + 1) % ring.length];
    return sum + point.x * next.z - next.x * point.z;
  }, 0) / 2;
}

function area(regions) {
  return regions.reduce((sum, item) => sum - signedArea(item.outer)
    - item.holes.reduce((holes, ring) => holes + signedArea(ring), 0), 0);
}

function spanOf(xs) {
  return `${Math.min(...xs)}..${Math.max(...xs)}`;
}

function nestOf(regions, xsOf) {
  return regions
    .map((item) => `${spanOf(xsOf(item.outer))} [${item.holes.map((ring) => spanOf(xsOf(ring))).sort().join(', ')}]`)
    .sort();
}

function pointXs(ring) {
  return ring.map((point) => point.x);
}

function edgeXs(ring) {
  return ring.flatMap((edge) => (edge.kind === 'line' ? [edge.from[0], edge.to[0]] : []));
}

function line(from, to) {
  return { kind: 'line', from, to };
}

function curvedRing(x0, z0, x1, z1) {
  const corners = [[x0, z0], [x0, z1], [x1, z1], [x1, z0]];
  return corners.map((corner, index) => line(corner, corners[(index + 1) % 4]));
}

function curvedRegion(x0, z0, x1, z1, hole) {
  const holes = hole ? [curvedRing(...hole).reverse().map((edge) => line(edge.to, edge.from))] : [];
  return { outer: curvedRing(x0, z0, x1, z1), holes };
}

function sector(start, end) {
  const at = (radius, angle) => [radius * Math.cos(angle), radius * Math.sin(angle)];
  const arc = (radius, from, to) => ({
    kind: 'arc', center: [0, 0], radius, start_angle: from, sweep_angle: to - from,
  });
  return {
    outer: [arc(2, end, start), line(at(2, start), at(1, start)), arc(1, start, end), line(at(1, end), at(2, end))],
    holes: [],
  };
}

test('two squares 0.05 mm apart unite to two regions', () => {
  const united = OpenGeometry.booleanRegions2D([region(0, 0, 1, 1)], [region(1.00005, 0, 2.00005, 1)], 'union');
  assert.equal(united.length, 2);
  for (const item of united) {
    assert.equal(item.outer.length, 4);
    assert.equal(item.holes.length, 0);
  }
});

test('an island in the hole of a frame keeps its own hole in every operand order', () => {
  const frame = region(0, 0, 10, 10, [1, 1, 9, 9]);
  const island = region(3, 3, 7, 7, [4, 4, 6, 6]);
  const expected = ['0..10 [1..9]', '3..7 [4..6]'];
  assert.deepEqual(nestOf(OpenGeometry.booleanRegions2D([frame], [island], 'union'), pointXs), expected);
  assert.deepEqual(nestOf(OpenGeometry.booleanRegions2D([island], [frame], 'union'), pointXs), expected);
  assert.deepEqual(nestOf(OpenGeometry.booleanRegions2D([frame, island], [], 'union'), pointXs), expected);
});

test('a boundary that cannot be closed is InvalidTopology with the 2.0 text', () => {
  const long = region(0, 0, 1000, 1);
  const besideItsEnd = region(999.9995, 1, 1000.5, 2);
  assert.throws(
    () => OpenGeometry.booleanRegions2D([long], [besideItsEnd], 'union'),
    (error) => failsWith('InvalidTopology')(error) && error instanceof Error
      && error.message.includes('Boolean regions produced an open boundary'),
  );
});

test('the curved boolean keeps an island in the hole of a frame apart in both operand orders', () => {
  const frame = curvedRegion(0, 0, 10, 10, [1, 1, 9, 9]);
  const island = curvedRegion(3, 3, 7, 7, [4, 4, 6, 6]);
  const expected = ['0..10 [1..9]', '3..7 [4..6]'];
  for (const [a, b] of [[frame, island], [island, frame]]) {
    assert.deepEqual(nestOf(OpenGeometry.booleanCurvedRegions2D(wire([a]), wire([b]), 'union'), edgeXs), expected);
  }
});

test('results come back at the input height, clockwise outside, counter-clockwise holes, from the smallest x', () => {
  const united = OpenGeometry.booleanRegions2D(
    [region(0, 0, 4, 4, [1, 1, 2, 2], 2.5)], [region(3, -1, 6, 2, undefined, 2.5)], 'union',
  );
  assert.equal(united.length, 1);
  const [only] = united;
  assert(only);
  assert([only.outer, ...only.holes].flat().every((point) => point.y === 2.5));
  assert(signedArea(only.outer) < 0);
  assert(only.holes.length === 1 && signedArea(only.holes[0]) > 0);
  for (const ring of [only.outer, ...only.holes]) {
    const [first] = ring;
    const smallest = Math.min(...pointXs(ring));
    assert(first && first.x === smallest);
    assert.equal(first.z, Math.min(...ring.filter((point) => point.x === smallest).map((point) => point.z)));
  }
  assert(Math.abs(area(united) - (16 + 9 - 2 - 1)) < 1e-9);
});

test('a wall at a three-way node loses what the earlier walls already hold', () => {
  const wall = region(0, -0.1, 5, 0.1);
  const previous = [region(-0.1, -3, 0.1, 3), region(-3, -0.1, 0.05, 0.1)];
  const remaining = OpenGeometry.booleanRegions2D([wall], previous, 'subtraction');
  assert.equal(remaining.length, 1);
  assert(Math.abs(area(remaining) - (1 - 0.02)) < 1e-9);
});

test('a curved wall cut and clipped by a straight wall keeps its arcs on their circles', () => {
  const curved = wire(sector(0, Math.PI / 2));
  const straight = wire(curvedRegion(1.2, -0.2, 1.8, 2.2));
  const cut = OpenGeometry.booleanCurvedRegions2D([curved], [straight], 'subtraction');
  const kept = OpenGeometry.booleanCurvedRegions2D([curved], [straight], 'intersection');
  assert.equal(cut.length, 2);
  assert.equal(kept.length, 1);
  const arcs = [...cut, ...kept].flatMap((item) => item.outer).filter((edge) => edge.kind === 'arc');
  assert(arcs.length >= 3);
  for (const arc of arcs) {
    assert.deepEqual(arc.center, [0, 0]);
    assert(arc.radius === 1 || arc.radius === 2);
  }
});

test('an overlap test reads the length of an intersection, and a result feeds back as an operand', () => {
  const host = region(0, 0, 4, 4);
  assert(OpenGeometry.booleanRegions2D([host], [region(3, 3, 5, 5)], 'intersection').length > 0);
  assert.equal(OpenGeometry.booleanRegions2D([host], [region(5, 5, 6, 6)], 'intersection').length, 0);
  const [first] = OpenGeometry.booleanRegions2D([host], [region(2, -1, 6, 2)], 'intersection');
  assert(first);
  const again = OpenGeometry.booleanRegions2D([first], [region(3, 0, 5, 5)], 'intersection');
  assert(Math.abs(area(again) - 2) < 1e-9);
  const curved = OpenGeometry.booleanCurvedRegions2D(wire([sector(0, 1)]), wire([sector(0.5, 1.5)]), 'intersection');
  assert(curved.length > 0);
  assert.equal(OpenGeometry.booleanCurvedRegions2D(curved, wire([sector(2, 3)]), 'intersection').length, 0);
});

test('bad input fails with InvalidParameter', () => {
  const square = region(0, 0, 1, 1);
  const reversed = { outer: [...square.outer].reverse(), holes: [] };
  const twoPoints = { outer: square.outer.slice(0, 2), holes: [] };
  const twoHeights = { outer: square.outer.map((point, index) => ({ ...point, y: index })), holes: [] };
  assert.throws(() => OpenGeometry.booleanRegions2D([square], [], wire('xor')), failsWith('InvalidParameter'));
  for (const input of [reversed, twoPoints, twoHeights, wire({ outer: square.outer })]) {
    assert.throws(() => OpenGeometry.booleanRegions2D([input], [], 'union'), failsWith('InvalidParameter'));
  }
  const extraField = wire({ ...square, colour: 'red' });
  const extraPointField = wire({ outer: square.outer.map((point) => ({ ...point, w: 1 })), holes: [] });
  for (const input of [extraField, extraPointField]) {
    assert.throws(() => OpenGeometry.booleanRegions2D([input], [], 'union'), failsWith('InvalidParameter'));
  }
  const extra = wire({ ...curvedRegion(0, 0, 1, 1), colour: 'red' });
  assert.throws(() => OpenGeometry.booleanCurvedRegions2D([extra], [], 'union'), failsWith('InvalidParameter'));
});

test('an operation outside the three words, or a curved operand that is not a list, is InvalidParameter', () => {
  const square = region(0, 0, 1, 1);
  const curved = wire([curvedRegion(0, 0, 1, 1)]);
  for (const operation of [wire(5), wire({}).operation]) {
    assert.throws(() => OpenGeometry.booleanRegions2D([square], [], operation), failsWith('InvalidParameter'));
    assert.throws(() => OpenGeometry.booleanCurvedRegions2D(curved, [], operation), failsWith('InvalidParameter'));
  }
  assert.throws(() => OpenGeometry.booleanCurvedRegions2D(curved, wire(null), 'union'), failsWith('InvalidParameter'));
  assert.equal(OpenGeometry.booleanRegions2D([square], [], 'union').length, 1);
});

test('a curved ring that does not close is UnresolvedIntersection', () => {
  const open = curvedRegion(0, 0, 1, 1);
  open.outer[2] = line([0.5, 1], [1, 1]);
  assert.throws(
    () => OpenGeometry.booleanCurvedRegions2D(wire([open]), [], 'union'), failsWith('UnresolvedIntersection'),
  );
});

test('more than 100,000 points across both operands is LimitExceeded', () => {
  const many = (offset) => ({
    outer: Array.from({ length: 50_001 }, (_, index) => {
      const angle = -2 * Math.PI * index / 50_001;
      return { x: offset + Math.cos(angle), y: 0, z: Math.sin(angle) };
    }),
    holes: [],
  });
  assert.throws(() => OpenGeometry.booleanRegions2D([many(0)], [many(5)], 'union'), failsWith('LimitExceeded'));
});

test('more than 100,000 edges across both curved operands is LimitExceeded', () => {
  const many = (offset) => {
    const corners = Array.from({ length: 50_001 }, (_, index) => {
      const angle = -2 * Math.PI * index / 50_001;
      return [offset + Math.cos(angle), Math.sin(angle)];
    });
    return { outer: corners.map((corner, index) => line(corner, corners[(index + 1) % corners.length])), holes: [] };
  };
  assert.throws(
    () => OpenGeometry.booleanCurvedRegions2D(wire([many(0)]), wire([many(5)]), 'union'), failsWith('LimitExceeded'),
  );
});

test('the tolerance is 1e-6, the same input gives the same output, and the graph does not change', () => {
  assert.equal(OG_REGION_TOLERANCE, 1e-6);
  const before = OpenGeometry.markStats();
  const geometry = listen('geometry');
  const a = [region(0, 0, 4, 4, [1, 1, 2, 2])];
  const b = [region(3, -1, 6, 2)];
  assert.deepEqual(OpenGeometry.booleanRegions2D(a, b, 'union'), OpenGeometry.booleanRegions2D(a, b, 'union'));
  OpenGeometry.flush({ geometry: 'sync' });
  geometry.stop();
  assert.equal(geometry.events.length, 0);
  assert.deepEqual(OpenGeometry.markStats(), before);
});
