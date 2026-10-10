import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { afterEach, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { AnalyticSolid, OpenGeometry } from "../../../dist/index.js";

const distFolder = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../dist");
const consumerAxes = { x: [1, 0, 0], y: [0, 0, -1], z: [0, 1, 0] };
const madeBodies = [];

before(async () => {
  if (typeof globalThis.document === "undefined") {
    globalThis.document = {
      createElement: () => ({ style: {}, setAttribute() {}, appendChild() {} }),
    };
  }
  const wasmBytes = await readFile(path.join(distFolder, "opengeometry", "pkg", "opengeometry_bg.wasm"));
  await OpenGeometry.create({ wasmURL: wasmBytes });
});

afterEach(() => {
  for (const body of madeBodies.splice(0)) body.dispose();
});

function keep(body) {
  madeBodies.push(body);
  return body;
}

function prism(left, bottom, width, depth, height, extra = {}) {
  return keep(new AnalyticSolid({
    kind: "linearExtrusion",
    frame: { origin: [0, 0, 0], ...consumerAxes },
    outer: [[left, bottom], [left + width, bottom], [left + width, bottom + depth], [left, bottom + depth]],
    holes: [],
    height,
    ...extra,
  }));
}

function wallAt(left, bottom, extra = {}) {
  return prism(left, bottom, 5, 0.2, 3, extra);
}

function movedOffIdentity(body) {
  body.position.y = 0.001;
  body.updateMatrixWorld(true);
  return body;
}

function assertRelative(actual, expected, relative, label) {
  assert.ok(
    Math.abs(actual - expected) <= relative * Math.abs(expected),
    `${label}: ${actual} is not ${expected} within ${relative} relative`,
  );
}

test("a wall gets the same default accuracy at every distance from the origin", (context) => {
  const atOrigin = wallAt(0, 0).getBrepData().accuracy;
  assertRelative(atOrigin.geometric, 5 * 1e-8, 1e-9, "wall at the origin");
  for (const [left, bottom] of [[100, 100], [5000, 5000], [1234.567, -4321.0123]]) {
    const accuracy = wallAt(left, bottom).getBrepData().accuracy;
    context.diagnostic(`wall at (${left}, ${bottom}): geometric ${accuracy.geometric}`);
    assertRelative(accuracy.geometric, atOrigin.geometric, 1e-9, `wall at (${left}, ${bottom}) against the origin`);
    assertRelative(accuracy.geometric, 5 * 1e-8, 1e-9, `wall at (${left}, ${bottom})`);
    assert.equal(accuracy.intersection, accuracy.geometric / 4);
  }
});

test("a body centred on the origin gets the accuracy of its full size", (context) => {
  const accuracy = prism(-2.6, -2.1, 5.2, 4.2, 0.2).getBrepData().accuracy;
  context.diagnostic(`slab 5.2 x 4.2 m centred on the origin: geometric ${accuracy.geometric}`);
  assertRelative(accuracy.geometric, 5.2 * 1e-8, 1e-12, "slab centred on the origin");
});

test("a small far body with a matrix passes the placement check of the kernel", (context) => {
  const body = movedOffIdentity(prism(50000, 50000, 0.05, 0.02, 0.05));
  const local = body.getBrepData().accuracy.geometric;
  const world = JSON.parse(body.getWorldBrepSerialized()).accuracy.geometric;
  context.diagnostic(`0.05 m prism at (50000, 50000): geometric ${local}`);
  assert.equal(world, local);
});

test("a cuboid far on the diagonal with a matrix passes the placement check of the kernel", (context) => {
  const body = movedOffIdentity(keep(new AnalyticSolid({
    kind: "cuboid",
    width: 1,
    depth: 1,
    height: 1,
    frame: { origin: [3e6, 0, 3e6], ...consumerAxes },
  })));
  const world = JSON.parse(body.getWorldBrepSerialized()).accuracy.geometric;
  context.diagnostic(`1 m cuboid at (3e6, 0, 3e6): geometric ${world}`);
  assert.equal(world, body.getBrepData().accuracy.geometric);
});

test("an explicit accuracy passes through unchanged", () => {
  const accuracy = { geometric: 1e-7, intersection: 2.5e-8, tessellation: 0.001, exchange: 0.00001 };
  assert.deepEqual(wallAt(5000, 5000, { accuracy }).getBrepData().accuracy, accuracy);
});

test("a door cut of a far wall equals the cut at the origin after the translation", (context) => {
  const cutAt = (left, bottom) => {
    const cutter = prism(left + 1.5, bottom - 0.1, 0.9, 0.4, 2.1);
    return keep(wallAt(left, bottom).subtract([cutter])).getBrepData().topology;
  };
  const near = cutAt(0, 0);
  const far = cutAt(5000, 5000);
  const translation = [5000, 0, -5000];
  assert.equal(far.faces.length, near.faces.length);
  assert.equal(far.edges.length, near.edges.length);
  assert.equal(far.vertices.length, near.vertices.length);
  let largest = 0;
  far.vertices.forEach((vertex, index) => {
    vertex.position.forEach((coordinate, axis) => {
      const difference = Math.abs(coordinate - translation[axis] - near.vertices[index].position[axis]);
      largest = Math.max(largest, difference);
    });
  });
  context.diagnostic(`largest vertex difference ${largest} m over ${near.vertices.length} vertices`);
  assert.ok(largest <= 1e-9, `largest vertex difference ${largest} m`);
});

const degrees = Math.PI / 180;

function curvedWall({ start, sweep, origin = [0, 0, 0], clockwise = false }) {
  const [inner, outer] = [80, 80.3];
  const at = (radius, angle) => [radius * Math.cos(angle), radius * Math.sin(angle)];
  const arc = (radius, startAngle, sweepAngle) => ({ kind: "arc", center: [0, 0], radius, startAngle, sweepAngle });
  const ring = [
    arc(outer, start, sweep),
    { kind: "line", from: at(outer, start + sweep), to: at(inner, start + sweep) },
    arc(inner, start + sweep, -sweep),
    { kind: "line", from: at(inner, start), to: at(outer, start) },
  ];
  const reversed = [...ring].reverse().map((edge) => (edge.kind === "line"
    ? { kind: "line", from: edge.to, to: edge.from }
    : arc(edge.radius, edge.startAngle + edge.sweepAngle, -edge.sweepAngle)));
  const body = keep(new AnalyticSolid({
    kind: "arcEdgedExtrusion",
    frame: { origin, ...consumerAxes },
    outer: clockwise ? reversed : ring,
    height: 3,
  }));
  return body.getBrepData().accuracy.geometric;
}

function boxRuleOf({ start, sweep, extremes }) {
  const [inner, outer] = [80, 80.3];
  const corners = [start, start + sweep].flatMap((angle) => [inner, outer].map(
    (radius) => [radius * Math.cos(angle), radius * Math.sin(angle)],
  ));
  const points = [...corners, ...extremes.map(([alongX, alongY]) => [alongX * outer, alongY * outer])];
  const side = (axis) => Math.max(...points.map((point) => point[axis])) - Math.min(...points.map((point) => point[axis]));
  return Math.max(3, side(0), side(1)) * 1e-8;
}

test("a gently curved wall gets the accuracy of its own box, near and far", (context) => {
  const wall = { start: 40 * degrees, sweep: 10 * degrees };
  const expected = boxRuleOf({ ...wall, extremes: [] });
  const near = curvedWall(wall);
  const far = curvedWall({ ...wall, origin: [5000, 0, 5000] });
  context.diagnostic(`radius 80 m, sweep 10 degrees: geometric ${near} near, ${far} at (5000, 5000)`);
  assertRelative(near, expected, 1e-9, "curved wall at the origin");
  assertRelative(far, expected, 1e-9, "curved wall at (5000, 5000)");
  assert.ok(near < 1e-6);
});

test("a short arc beside an axis extreme gets the box of the arc", (context) => {
  const cases = [
    { start: 80 * degrees, sweep: 20 * degrees, extremes: [[0, 1]] },
    { start: -10 * degrees, sweep: 20 * degrees, extremes: [[1, 0]] },
    { start: 170 * degrees, sweep: 20 * degrees, extremes: [[-1, 0]] },
    { start: 260 * degrees, sweep: 20 * degrees, extremes: [[0, -1]] },
  ];
  for (const wall of cases) {
    const geometric = curvedWall(wall);
    context.diagnostic(`sweep from ${Math.round(wall.start / degrees)} degrees: geometric ${geometric}`);
    assertRelative(geometric, boxRuleOf(wall), 1e-9, `sweep from ${Math.round(wall.start / degrees)} degrees`);
  }
});

test("an arc of more than a half turn is as wide as its circle where it passes both extremes", (context) => {
  const wall = { start: -135 * degrees, sweep: 270 * degrees, extremes: [[1, 0], [0, 1], [0, -1]] };
  const geometric = curvedWall(wall);
  context.diagnostic(`sweep of 270 degrees: geometric ${geometric}`);
  assertRelative(geometric, 2 * 80.3 * 1e-8, 1e-9, "sweep of 270 degrees against the circle");
  assertRelative(geometric, boxRuleOf(wall), 1e-9, "sweep of 270 degrees");
});

test("an arc gives the same box in both directions", () => {
  for (const wall of [{ start: 40 * degrees, sweep: 10 * degrees, extremes: [] }, { start: 80 * degrees, sweep: 20 * degrees, extremes: [[0, 1]] }]) {
    const expected = boxRuleOf(wall);
    assertRelative(curvedWall(wall), expected, 1e-9, "counter-clockwise ring");
    assertRelative(curvedWall({ ...wall, clockwise: true }), expected, 1e-9, "clockwise ring");
  }
});

test("the box of an arc reaches each axis extreme that its sweep passes", (context) => {
  const [radius, halfSweep, depth] = [80, 10 * degrees, 200];
  const missed = [];
  for (const quarter of [0, 1, 2, 3]) {
    const middle = quarter * 90 * degrees;
    const turned = ([along, across]) => [
      along * Math.cos(middle) - across * Math.sin(middle),
      along * Math.sin(middle) + across * Math.cos(middle),
    ];
    const halfChord = radius * Math.sin(halfSweep);
    const arcEnd = (side) => turned([radius * Math.cos(halfSweep), side * halfChord]);
    const body = keep(new AnalyticSolid({
      kind: "arcEdgedExtrusion",
      frame: { origin: [0, 0, 0], ...consumerAxes },
      outer: [
        { kind: "arc", center: [0, 0], radius, startAngle: middle - halfSweep, sweepAngle: 2 * halfSweep },
        { kind: "line", from: arcEnd(1), to: turned([-depth, halfChord]) },
        { kind: "line", from: turned([-depth, halfChord]), to: turned([-depth, -halfChord]) },
        { kind: "line", from: turned([-depth, -halfChord]), to: arcEnd(-1) },
      ],
      height: 3,
    }));
    const geometric = body.getBrepData().accuracy.geometric;
    context.diagnostic(`arc across the extreme at ${quarter * 90} degrees: geometric ${geometric}`);
    if (Math.abs(geometric - (radius + depth) * 1e-8) > 1e-9 * geometric) missed.push(quarter * 90);
  }
  assert.deepEqual(missed, []);
});
