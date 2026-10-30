import assert from 'node:assert/strict';
import { test } from 'node:test';
import { bootKernel, MEBIBYTE, PAGE_BYTES, readKernelBytes } from './kernel-memory.mjs';
import { boot, cuboid } from './support.mjs';

const BODY_COUNT = 5000;
const RECORDED = { growthSteps: 22, pagesAfterAllBodies: 1033, pagesAfterOneBody: 36 };
const MOST_GROWTH_STEPS = 100;
const STEP_MARGIN = 0.1;
const PAGE_MARGIN = 0.05;
const COMPARED = process.env.CI !== 'true';
const KEPT_BODIES = [];

async function measure() {
  const measured = { startPages: 0, afterOneBody: 0, afterAllBodies: 0, growthSteps: 0 };
  const kernel = await bootKernel(boot);
  measured.startPages = kernel.pages();
  let lastPages = measured.startPages;
  for (let index = 0; index < BODY_COUNT; index += 1) {
    KEPT_BODIES.push(cuboid(`body-${index}`));
    if (kernel.pages() !== lastPages) {
      measured.growthSteps += 1;
      lastPages = kernel.pages();
    }
    if (index === 0) measured.afterOneBody = lastPages;
  }
  measured.afterAllBodies = lastPages;
  return measured;
}

const MEASURED = await measure();

function reader(bytes) {
  const read = { position: 8 };
  read.byte = () => {
    read.position += 1;
    return bytes[read.position - 1];
  };
  read.unsigned = () => {
    let value = 0;
    let scale = 1;
    let byte;
    do {
      byte = read.byte();
      value += (byte & 0x7f) * scale;
      scale *= 128;
    } while (byte & 0x80);
    return value;
  };
  read.limits = () => {
    const flags = read.byte();
    const initial = read.unsigned();
    return { flags, initial, maximum: flags & 1 ? read.unsigned() : undefined };
  };
  return read;
}

function addImportedMemories(read, importedMemories) {
  for (let count = read.unsigned(); count > 0; count -= 1) {
    for (let name = 0; name < 2; name += 1) {
      const length = read.unsigned();
      read.position += length;
    }
    const kind = read.byte();
    if (kind === 0) read.unsigned();
    if (kind === 1) {
      read.byte();
      read.limits();
    }
    if (kind === 2) importedMemories.push(read.limits());
    if (kind === 3) read.position += 2;
  }
}

function sectionsOf(bytes) {
  const read = reader(bytes);
  const memories = [];
  const importedMemories = [];
  while (read.position < bytes.length) {
    const id = read.byte();
    const end = read.unsigned() + read.position;
    if (id === 5) {
      for (let count = read.unsigned(); count > 0; count -= 1) memories.push(read.limits());
    }
    if (id === 2) addImportedMemories(read, importedMemories);
    read.position = end;
  }
  return { memories, importedMemories };
}

function report(line) {
  process.stdout.write(`# ${line}\n`);
}

function mebibytes(pages) {
  return (pages * PAGE_BYTES / MEBIBYTE).toFixed(2);
}

function assertNear(actual, recorded, margin, name) {
  const low = Math.floor(recorded * (1 - margin));
  const high = Math.ceil(recorded * (1 + margin));
  assert(actual >= low && actual <= high, `${name} ${actual} outside ${low}..${high} (recorded ${recorded})`);
}

test('the kernel has one memory of its own, not shared and without a maximum', () => {
  const { memories, importedMemories } = sectionsOf(readKernelBytes());
  assert.equal(importedMemories.length, 0);
  assert.equal(memories.length, 1);
  assert.equal(memories[0].flags & 2, 0);
  assert.equal(memories[0].maximum, undefined);
});

test('the section reader finds the imported and exported memories the engine finds', () => {
  const bytes = readKernelBytes();
  const module = new WebAssembly.Module(bytes);
  const { memories, importedMemories } = sectionsOf(bytes);
  const memoryCount = (entries) => entries.filter((entry) => entry.kind === 'memory').length;
  assert.equal(importedMemories.length, memoryCount(WebAssembly.Module.imports(module)));
  assert.equal(memories.length, memoryCount(WebAssembly.Module.exports(module)));
});

test('keeping 5,000 cuboids grows the memory at most 100 times, the recorded number of times', () => {
  const { growthSteps, startPages, afterAllBodies } = MEASURED;
  report(`${BODY_COUNT} kept cuboids: ${growthSteps} growth steps`);
  report(`${startPages} to ${afterAllBodies} pages (${mebibytes(afterAllBodies)} MiB)`);
  assert(MEASURED.growthSteps <= MEASURED.afterAllBodies - MEASURED.startPages);
  assert(MEASURED.growthSteps <= MOST_GROWTH_STEPS, `${MEASURED.growthSteps} growth steps`);
  if (COMPARED) assertNear(MEASURED.growthSteps, RECORDED.growthSteps, STEP_MARGIN, 'growth steps');
});

test('5,000 kept cuboids need the recorded page total', () => {
  const perBody = (MEASURED.afterAllBodies - MEASURED.startPages) * PAGE_BYTES / BODY_COUNT;
  report(`about ${Math.round(perBody)} bytes of memory per kept cuboid`);
  assert(MEASURED.afterAllBodies >= MEASURED.afterOneBody);
  if (COMPARED) assertNear(MEASURED.afterAllBodies, RECORDED.pagesAfterAllBodies, PAGE_MARGIN, 'pages');
});

test('one kept cuboid needs at most 4 MiB of memory', () => {
  report(`one cuboid: ${MEASURED.afterOneBody} pages (${mebibytes(MEASURED.afterOneBody)} MiB)`);
  assert(MEASURED.afterOneBody * PAGE_BYTES <= 4 * MEBIBYTE, `${MEASURED.afterOneBody} pages`);
  if (COMPARED) assertNear(MEASURED.afterOneBody, RECORDED.pagesAfterOneBody, PAGE_MARGIN, 'pages after one cuboid');
});
