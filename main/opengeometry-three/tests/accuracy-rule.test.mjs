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
