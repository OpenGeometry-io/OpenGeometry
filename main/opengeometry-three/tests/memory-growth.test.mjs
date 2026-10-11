import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { bootKernel, MEBIBYTE, PAGE_BYTES } from "./kernel-memory.mjs";

const BODY_COUNT = 5000;
const BYTES_AFTER_THE_BODIES_WITHOUT_A_RESERVE = 55508992;
const keptBodies = [];
const measured = { wasmBytes: undefined, afterOneBody: 0, afterAllBodies: 0, growthSteps: 0 };

before(async () => {
  const kernel = await bootKernel();
  measured.wasmBytes = kernel.wasmBytes;
  let lastPages = kernel.pages();
  for (let index = 0; index < BODY_COUNT; index += 1) {
    keptBodies.push(kernel.glue.OGAnalyticBrep.from_primitive(JSON.stringify({
      kind: "cuboid",
      id: `body-${index}`,
      frame: { origin: [0, 0, 0], x: [1, 0, 0], y: [0, 0, -1], z: [0, 1, 0] },
      accuracy: { geometric: 1e-9, intersection: 1e-10, tessellation: 0.001, exchange: 1e-5 },
      size: [4, 3, 0.2],
    })));
    if (kernel.pages() !== lastPages) {
      measured.growthSteps += 1;
      lastPages = kernel.pages();
    }
    if (index === 0) measured.afterOneBody = lastPages * PAGE_BYTES;
  }
  measured.afterAllBodies = lastPages * PAGE_BYTES;
});

after(() => {
  for (const body of keptBodies.splice(0)) body.free();
});

function sectionsOf(bytes) {
  let position = 8;
  const unsigned = () => {
    let value = 0;
    let scale = 1;
    let byte;
    do {
      byte = bytes[position];
      position += 1;
      value += (byte & 0x7f) * scale;
      scale *= 128;
    } while (byte & 0x80);
    return value;
  };
  const limits = () => {
    const flags = bytes[position];
    position += 1;
    const initial = unsigned();
    return { flags, initial, maximum: flags & 1 ? unsigned() : undefined };
  };
  const memories = [];
  const importedMemories = [];
  while (position < bytes.length) {
    const id = bytes[position];
    position += 1;
    const size = unsigned();
    const end = position + size;
    if (id === 5) {
      for (let count = unsigned(); count > 0; count -= 1) memories.push(limits());
    }
    if (id === 2) {
      for (let count = unsigned(); count > 0; count -= 1) {
        const moduleNameLength = unsigned();
        position += moduleNameLength;
        const fieldNameLength = unsigned();
        position += fieldNameLength;
        const kind = bytes[position];
        position += 1;
        if (kind === 0) unsigned();
        if (kind === 1) {
          position += 1;
          limits();
        }
        if (kind === 2) importedMemories.push(limits());
        if (kind === 3) position += 2;
      }
    }
    position = end;
  }
  return { memories, importedMemories };
}

test("the kernel has one memory of its own, not shared and without a maximum", () => {
  const { memories, importedMemories } = sectionsOf(measured.wasmBytes);
  assert.equal(importedMemories.length, 0);
  assert.equal(memories.length, 1);
  assert.equal(memories[0].flags & 2, 0);
  assert.equal(memories[0].maximum, undefined);
});

test("the memory grows in large steps", (context) => {
  context.diagnostic(`${BODY_COUNT} kept bodies: ${measured.growthSteps} growth steps, ${(measured.afterAllBodies / MEBIBYTE).toFixed(2)} MiB (${measured.afterAllBodies} bytes)`);
  assert.ok(measured.growthSteps <= 100, `${measured.growthSteps} growth steps`);
});

test("many small kept bodies need at most an eighth and one step more memory than without a reserve", () => {
  const bound = 1.125 * BYTES_AFTER_THE_BODIES_WITHOUT_A_RESERVE + MEBIBYTE + PAGE_BYTES;
  assert.ok(measured.afterAllBodies <= bound, `${measured.afterAllBodies} bytes against a bound of ${bound}`);
});

test("one body needs at most 4 MiB of memory", (context) => {
  context.diagnostic(`one body: ${(measured.afterOneBody / MEBIBYTE).toFixed(2)} MiB`);
  assert.ok(measured.afterOneBody <= 4 * MEBIBYTE, `${measured.afterOneBody} bytes`);
});
