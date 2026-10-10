import assert from "node:assert/strict";
import { test } from "node:test";
import { bootKernel, MEBIBYTE, PAGE_BYTES } from "./kernel-memory.mjs";

const LIMIT_PAGES = 65536;

test("a refused reserve leaves the kernel working up to the last page of the engine", async (context) => {
  const kernel = await bootKernel();
  const pagesLeft = () => LIMIT_PAGES - kernel.pages();
  try {
    while (pagesLeft() > 2100) {
      kernel.allocate(Math.min(1024 * MEBIBYTE, (pagesLeft() - 2090) * PAGE_BYTES));
    }
  } catch (error) {
    context.skip(`this engine does not give 4 GiB of address space: ${String(error)}`);
    return;
  }
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
  const pagesAtTheRefusal = kernel.pages();
  const growthSteps = [];
  let pagesBefore = pagesAtTheRefusal;
  let smallBlocks = 0;
  while (kernel.pages() < LIMIT_PAGES) {
    kernel.allocate(1024);
    smallBlocks += 1;
    if (kernel.pages() !== pagesBefore) {
      growthSteps.push(kernel.pages() - pagesBefore);
      pagesBefore = kernel.pages();
    }
  }
  context.diagnostic(`${pagesAtTheRefusal} pages before the last small blocks, ${smallBlocks} blocks of 1 KiB, growth steps ${JSON.stringify(growthSteps)}, end ${kernel.pages()} pages`);
  assert.ok(pagesAtTheRefusal > LIMIT_PAGES - 1024 && pagesAtTheRefusal < LIMIT_PAGES);
  assert.deepEqual(growthSteps, new Array(LIMIT_PAGES - pagesAtTheRefusal).fill(1));
  assert.equal(kernel.pages(), LIMIT_PAGES);
  assert.notEqual(kernel.allocate(1024), 0);
});
