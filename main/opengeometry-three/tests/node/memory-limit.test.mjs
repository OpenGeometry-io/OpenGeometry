import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MEBIBYTE, PAGE_BYTES, rawKernel } from './kernel-memory.mjs';

const LIMIT_PAGES = 65536;

function fillTheTop(kernel, pagesLeft) {
  while (pagesLeft() > 1023) kernel.allocate((pagesLeft() - 1000) * PAGE_BYTES);
  for (const bytes of [16 * MEBIBYTE, 4 * MEBIBYTE, MEBIBYTE, 256 * 1024, 64 * 1024, 16 * 1024, 4096]) {
    let fitsTheFreeTop = true;
    while (fitsTheFreeTop && bytes / PAGE_BYTES + 2 <= pagesLeft()) {
      const pagesBefore = kernel.pages();
      kernel.allocate(bytes);
      fitsTheFreeTop = kernel.pages() === pagesBefore;
    }
  }
  if (pagesLeft() > 10) kernel.allocate((pagesLeft() - 8) * PAGE_BYTES);
}

function growthStepsToTheLimit(kernel) {
  const growthSteps = [];
  let pagesBefore = kernel.pages();
  while (kernel.pages() < LIMIT_PAGES) {
    kernel.allocate(1024);
    if (kernel.pages() !== pagesBefore) {
      growthSteps.push(kernel.pages() - pagesBefore);
      pagesBefore = kernel.pages();
    }
  }
  return growthSteps;
}

function missingAddressSpace() {
  try {
    new WebAssembly.Memory({ initial: 0 }).grow(LIMIT_PAGES);
    return false;
  } catch (error) {
    return `this engine does not give 4 GiB of address space: ${String(error)}`;
  }
}

const NAME = 'a refused reserve leaves the kernel working up to the last page of the engine';

test(NAME, { skip: missingAddressSpace() }, async () => {
  const kernel = await rawKernel();
  const pagesLeft = () => LIMIT_PAGES - kernel.pages();
  while (pagesLeft() > 2100) {
    kernel.allocate(Math.min(1024 * MEBIBYTE, (pagesLeft() - 2090) * PAGE_BYTES));
  }
  fillTheTop(kernel, pagesLeft);
  const pagesAtTheRefusal = kernel.pages();
  const growthSteps = growthStepsToTheLimit(kernel);
  process.stdout.write(`# ${pagesAtTheRefusal} pages before the last small blocks, ${growthSteps.length} steps\n`);
  assert(pagesAtTheRefusal > LIMIT_PAGES - 1024 && pagesAtTheRefusal < LIMIT_PAGES);
  assert.deepEqual(growthSteps, new Array(LIMIT_PAGES - pagesAtTheRefusal).fill(1));
  assert.equal(kernel.pages(), LIMIT_PAGES);
  assert.notEqual(kernel.allocate(1024), 0);
});
