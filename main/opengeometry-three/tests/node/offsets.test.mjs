import assert from 'node:assert/strict';
import { test } from 'node:test';
import { OpenGeometry } from '../../../dist/index.js';
import { boot, failsWith, listen } from './support.mjs';

await boot();

function wire(value) {
  return JSON.parse(JSON.stringify(value));
}

function flat(points, y = 0) {
  return points.flatMap(([x, z]) => [x, y, z]);
}

function ends(edge) {
  if (edge.kind === 'line') return [edge.from, edge.to];
  const at = (angle) => [
    edge.center[0] + edge.radius * Math.cos(angle), edge.center[1] + edge.radius * Math.sin(angle),
  ];
  return [at(edge.start_angle), at(edge.start_angle + edge.sweep_angle)];
}

function ringArea(edges) {
  return edges.reduce((sum, edge) => {
    const [a, b] = ends(edge);
    if (edge.kind === 'line') return sum + (a[0] * b[1] - b[0] * a[1]) / 2;
    const { center, radius, sweep_angle: sweep } = edge;
    return sum + (radius * radius * sweep + center[0] * (b[1] - a[1]) - center[1] * (b[0] - a[0])) / 2;
  }, 0);
}

function area(regions) {
  return regions.reduce((sum, region) => sum + Math.abs(ringArea(region.outer))
    - region.holes.reduce((holes, ring) => holes + Math.abs(ringArea(ring)), 0), 0);
}

function arcs(regions) {
  return regions.flatMap((region) => [region.outer, ...region.holes].flat()).filter((edge) => edge.kind === 'arc');
}

function near(a, b, tolerance = 1e-9) {
  return Math.abs(a - b) <= tolerance;
}

const SQUARE_LOT = [[0, 0], [20, 0], [20, 20], [0, 20]];
const HOLE = [[9, 9], [11, 9], [11, 11], [9, 11]];
const L_LOT = [[0, 0], [10, 0], [10, 5], [5, 5], [5, 10], [0, 10]];

test('an envelope inset is exact, with each hole corner rounded by its own distance', () => {
  const y = 2;
  const regions = OpenGeometry.offsetRingVariable(
    flat(SQUARE_LOT, y),
    [5, 3, 3, 3],
    [{ ring: flat(HOLE, y), distances: [1, 1, 1, 1] }],
  );
  const [only] = regions;
  assert(only && regions.length === 1);
  assert.equal(only.holes.length, 1);
  assert(near(area(regions), 12 * 14 - (12 + Math.PI)));
  const rounded = arcs(regions);
  assert.equal(rounded.length, 4);
  for (const arc of rounded) {
    assert(near(arc.radius, 1));
    assert(HOLE.some(([x, z]) => near(arc.center[0], x) && near(arc.center[1], z)));
  }
});

test('an inside lot corner insets to an exact arc of the setback round the corner', () => {
  const setbacks = [2, 2, 2, 2, 2, 2].map((inset) => (inset > 0 ? Math.max(inset, 0.001) : 0));
  const regions = OpenGeometry.offsetRingVariable(flat(L_LOT), setbacks);
  assert.equal(regions.length, 1);
  assert(near(area(regions), 15 - Math.PI));
  const [corner] = arcs(regions);
  assert.equal(arcs(regions).length, 1);
  assert(near(corner.radius, 2) && near(corner.center[0], 5) && near(corner.center[1], 5));
  assert(near(Math.abs(corner.sweep_angle), Math.PI / 2));
});

test('the ring height is ignored, and a typed array gives the same result as a list', () => {
  const low = OpenGeometry.offsetRingVariable(flat(L_LOT, 0), new Array(6).fill(1));
  const high = OpenGeometry.offsetRingVariable(new Float64Array(flat(L_LOT, 7)), new Float64Array(6).fill(1));
  assert.deepEqual(high, low);
});

test('a routed duct run strokes to width times length, mitred at the default limit of 4 and bevelled at 1', () => {
  const run = flat([[0, 0], [4, 0], [4, 3], [8, 3]]);
  const regions = OpenGeometry.offsetPolylineRegions(run, 0.3, false, 4);
  const [duct] = regions;
  assert(duct && regions.length === 1);
  assert(near(area(regions), 0.3 * 11));
  assert(duct.outer.every((edge) => edge.kind === 'line'));
  assert.equal(duct.outer.length, 12);
  assert.deepEqual(OpenGeometry.offsetPolylineRegions(run, 0.3, false), regions);
  const bevelled = OpenGeometry.offsetPolylineRegions(run, 0.3, false, 1);
  const [cut] = bevelled;
  assert(cut && bevelled.length === 1);
  assert.equal(cut.outer.length, 10);
  assert(area(bevelled) < 0.3 * 11 - 1e-9);
});

test('a wide loop is solid, a short-legged U and a self-crossing open path have their true areas', () => {
  const loop = OpenGeometry.offsetPolylineRegions(flat([[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]), 1.2, true);
  const [solid] = loop;
  assert(solid && loop.length === 1);
  assert.equal(solid.holes.length, 0);
  assert(near(area(loop), 4.84));
  const u = OpenGeometry.offsetPolylineRegions(flat([[0, 0], [10, 0], [10, 1], [0, 1]]), 2, false);
  assert(near(area(u), 33));
  const crossing = OpenGeometry.offsetPolylineRegions(flat([[0, 0], [4, 0], [4, 2], [2, 2], [2, -2]]), 0.4, false);
  const [crossed] = crossing;
  assert(crossed && crossing.length === 1);
  assert.equal(crossed.holes.length, 1);
  assert(near(area(crossing), 0.4 * 12 - 0.16));
});

test('a one-sided offset is parallel, bevels a corner below its acute threshold and keeps a closed path closed', () => {
  const line = OpenGeometry.offsetPolyline(flat([[0, 0], [5, 0]]), 1);
  assert.equal(line.closed, false);
  assert.deepEqual(line.bevelled, []);
  assert.deepEqual(line.edges, [{ kind: 'line', from: [0, 1], to: [5, 1] }]);
  const acute = OpenGeometry.offsetPolyline(flat([[0, 0], [2, 0], [1, 0.3]]), 0.3, { acuteThresholdDegrees: 45 });
  assert.deepEqual(acute.bevelled, [1]);
  assert(acute.edges.length >= 3);
  const wide = OpenGeometry.offsetPolyline(flat([[0, 0], [2, 0], [1, 0.3]]), 0.3, { acuteThresholdDegrees: 10 });
  assert.deepEqual(wide.bevelled, []);
  const mitred = OpenGeometry.offsetPolyline(flat([[0, 0], [2, 0], [1, 0.3]]), 0.3, {
    acuteThresholdDegrees: 45, bevel: false,
  });
  assert.deepEqual(mitred.bevelled, []);
  const square = flat([[0, 0], [2, 0], [2, 2], [0, 2], [0, 0]]);
  assert.equal(OpenGeometry.offsetPolyline(square, 0.2).closed, true);
  assert.equal(OpenGeometry.offsetPolyline(square, 0.2, { closed: false }).closed, false);
});

test('a bad distance count, a negative or non-finite distance and a non-finite width are InvalidParameter', () => {
  const ring = flat(SQUARE_LOT);
  for (const distances of [[1, 1, 1], [1, -1, 1, 1], [1, Number.NaN, 1, 1]]) {
    assert.throws(() => OpenGeometry.offsetRingVariable(ring, distances), failsWith('InvalidParameter'));
  }
  for (const width of [Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(() => OpenGeometry.offsetPolylineRegions(ring, width, true), failsWith('InvalidParameter'));
  }
  assert.throws(() => OpenGeometry.offsetPolyline(ring, Number.NaN), failsWith('InvalidParameter'));
});

test('a bowtie lot or a degenerate hole is InvalidTopology, and a collapsed inset is empty', () => {
  const bowtie = flat([[0, 0], [10, 10], [10, 0], [0, 10]]);
  assert.throws(() => OpenGeometry.offsetRingVariable(bowtie, [1, 1, 1, 1]), failsWith('InvalidTopology'));
  const flatHole = { ring: flat([[5, 5], [6, 5], [7, 5]]), distances: [1, 1, 1] };
  assert.throws(
    () => OpenGeometry.offsetRingVariable(flat(SQUARE_LOT), [1, 1, 1, 1], [flatHole]), failsWith('InvalidTopology'),
  );
  assert.deepEqual(OpenGeometry.offsetRingVariable(flat([[0, 0], [2, 0], [2, 2], [0, 2]]), [1.5, 1.5, 1.5, 1.5]), []);
  assert.deepEqual(OpenGeometry.offsetPolylineRegions(flat([[0, 0], [1, 0]]), 0, false), []);
});

test('input of the wrong shape is InvalidParameter before the kernel sees it', () => {
  const ring = flat(SQUARE_LOT);
  const distances = [1, 1, 1, 1];
  const hole = { ring: flat(HOLE), distances: [1, 1, 1, 1] };
  for (const [badRing, badDistances, badHoles] of [
    [wire('ring'), distances, []],
    [[0, 0, 'x'], distances, []],
    [ring, wire({ length: 4 }), []],
    [ring, distances, wire({})],
    [ring, distances, [wire({ ...hole, colour: 'red' })]],
    [ring, distances, [wire({ ring: hole.ring })]],
    [ring, distances, [wire(null)]],
  ]) {
    assert.throws(
      () => OpenGeometry.offsetRingVariable(badRing, badDistances, badHoles), failsWith('InvalidParameter'),
    );
  }
  assert.throws(() => OpenGeometry.offsetPolylineRegions(ring, wire('0.3'), false), failsWith('InvalidParameter'));
  assert.throws(() => OpenGeometry.offsetPolylineRegions(ring, 0.3, wire('no')), failsWith('InvalidParameter'));
  assert.throws(() => OpenGeometry.offsetPolylineRegions(ring, 0.3, false, wire('4')), failsWith('InvalidParameter'));
  for (const options of [wire({ colour: 'red' }), wire({ bevel: 'yes' }), wire({ closed: 1 }), wire(null)]) {
    assert.throws(() => OpenGeometry.offsetPolyline(ring, 0.3, options), failsWith('InvalidParameter'));
  }
  assert.throws(() => OpenGeometry.offsetPolyline(wire({}), 0.3), failsWith('InvalidParameter'));
});

test('more than 100,000 points is LimitExceeded', () => {
  const many = Array.from({ length: 100_001 }, (_, index) => {
    const angle = -2 * Math.PI * index / 100_001;
    return [100 * Math.cos(angle), 100 * Math.sin(angle)];
  });
  assert.throws(
    () => OpenGeometry.offsetRingVariable(flat(many), new Array(many.length).fill(1)), failsWith('LimitExceeded'),
  );
  assert.throws(() => OpenGeometry.offsetPolylineRegions(flat(many), 1, true), failsWith('LimitExceeded'));
  assert.throws(() => OpenGeometry.offsetPolyline(flat(many), 1), failsWith('LimitExceeded'));
});

test('the same input gives the same output, and the graph does not change', () => {
  const before = OpenGeometry.markStats();
  const geometry = listen('geometry');
  const inset = () => OpenGeometry.offsetRingVariable(flat(L_LOT), [2, 1, 3, 1, 2, 1]);
  assert.deepEqual(inset(), inset());
  OpenGeometry.offsetPolylineRegions(flat([[0, 0], [4, 0], [4, 3]]), 0.3, false);
  OpenGeometry.offsetPolyline(flat([[0, 0], [4, 0], [4, 3]]), 0.3);
  OpenGeometry.flush({ geometry: 'sync' });
  geometry.stop();
  assert.equal(geometry.events.length, 0);
  assert.deepEqual(OpenGeometry.markStats(), before);
});
