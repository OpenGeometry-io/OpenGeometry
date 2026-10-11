import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { booleanRegions2D, OpenGeometry } from "../../../dist/index.js";

const distFolder = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../dist");

before(async () => {
  if (typeof globalThis.document === "undefined") {
    globalThis.document = {
      createElement: () => ({ style: {}, setAttribute() {}, appendChild() {} }),
    };
  }
  const wasmBytes = await readFile(path.join(distFolder, "opengeometry", "pkg", "opengeometry_bg.wasm"));
  await OpenGeometry.create({ wasmURL: wasmBytes });
});

function clockwise(x0, z0, x1, z1) {
  return [[x0, z0], [x0, z1], [x1, z1], [x1, z0]].map(([x, z]) => ({ x, y: 0, z }));
}

function region(x0, z0, x1, z1, hole) {
  return { outer: clockwise(x0, z0, x1, z1), holes: hole ? [clockwise(...hole).reverse()] : [] };
}

function spanOf(ring) {
  const along = ring.map((point) => point.x);
  return `${Math.min(...along)}..${Math.max(...along)}`;
}

function nestOf(regions) {
  return regions
    .map((item) => `${spanOf(item.outer)} [${item.holes.map(spanOf).sort().join(", ")}]`)
    .sort();
}

test("two squares 0.05 mm apart unite to two regions", () => {
  const united = booleanRegions2D([region(0, 0, 1, 1)], [region(1.00005, 0, 2.00005, 1)], "union");
  assert.equal(united.length, 2);
  for (const item of united) {
    assert.equal(item.outer.length, 4);
    assert.equal(item.holes.length, 0);
  }
});

test("an island in the hole of a frame keeps its own hole in both operand orders", () => {
  const frame = region(0, 0, 10, 10, [1, 1, 9, 9]);
  const island = region(3, 3, 7, 7, [4, 4, 6, 6]);
  const expected = ["0..10 [1..9]", "3..7 [4..6]"];
  assert.deepEqual(nestOf(booleanRegions2D([frame], [island], "union")), expected);
  assert.deepEqual(nestOf(booleanRegions2D([island], [frame], "union")), expected);
  assert.deepEqual(nestOf(booleanRegions2D([frame, island], [], "union")), expected);
});

test("a boundary that cannot be closed is an error and not a ring", () => {
  const long = region(0, 0, 1000, 1);
  const besideItsEnd = region(999.9995, 1, 1000.5, 2);
  assert.throws(
    () => booleanRegions2D([long], [besideItsEnd], "union"),
    (error) => String(error).includes("Boolean regions produced an open boundary"),
  );
});
