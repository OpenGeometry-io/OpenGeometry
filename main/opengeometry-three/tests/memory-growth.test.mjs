import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageFolder = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../dist/opengeometry/pkg");
const PAGE_BYTES = 65536;
const MEBIBYTE = 1024 * 1024;
const BODY_COUNT = 5000;
const BYTES_AFTER_THE_BODIES_WITHOUT_A_RESERVE = 55508992;
const keptBodies = [];
const measured = { wasmBytes: undefined, afterOneBody: 0, afterAllBodies: 0, growthSteps: 0 };

before(async () => {
  measured.wasmBytes = await readFile(path.join(packageFolder, "opengeometry_bg.wasm"));
  const glue = await import(pathToFileURL(path.join(packageFolder, "opengeometry.js")).href);
  let memory;
  const instantiate = globalThis.WebAssembly.instantiate;
  globalThis.WebAssembly.instantiate = async (...parameters) => {
    const result = await instantiate(...parameters);
    memory = (result.instance ?? result).exports.memory;
    return result;
  };
  try {
    await glue.default({ module_or_path: measured.wasmBytes });
  } finally {
    globalThis.WebAssembly.instantiate = instantiate;
  }
  let lastBytes = memory.buffer.byteLength;
  for (let index = 0; index < BODY_COUNT; index += 1) {
    keptBodies.push(glue.OGAnalyticBrep.from_primitive(JSON.stringify({
      kind: "cuboid",
      id: `body-${index}`,
      frame: { origin: [0, 0, 0], x: [1, 0, 0], y: [0, 0, -1], z: [0, 1, 0] },
      accuracy: { geometric: 1e-9, intersection: 1e-10, tessellation: 0.001, exchange: 1e-5 },
      size: [4, 3, 0.2],
    })));
    if (memory.buffer.byteLength !== lastBytes) {
      measured.growthSteps += 1;
      lastBytes = memory.buffer.byteLength;
    }
    if (index === 0) measured.afterOneBody = lastBytes;
  }
  measured.afterAllBodies = lastBytes;
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

test("the reserve adds at most an eighth and one step to the memory", () => {
  const bound = 1.125 * BYTES_AFTER_THE_BODIES_WITHOUT_A_RESERVE + MEBIBYTE + PAGE_BYTES;
  assert.ok(measured.afterAllBodies <= bound, `${measured.afterAllBodies} bytes against a bound of ${bound}`);
});

test("one body needs at most 4 MiB of memory", (context) => {
  context.diagnostic(`one body: ${(measured.afterOneBody / MEBIBYTE).toFixed(2)} MiB`);
  assert.ok(measured.afterOneBody <= 4 * MEBIBYTE, `${measured.afterOneBody} bytes`);
});
