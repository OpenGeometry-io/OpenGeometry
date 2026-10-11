import assert from "node:assert/strict";
import { test } from "node:test";
import { bootKernel, MEBIBYTE, PAGE_BYTES, reserveBytes } from "./kernel-memory.mjs";

const roundedUp = (value, step) => Math.ceil(value / step) * step;

function memoryWithoutAReserve(startPages) {
  const heap = { pages: startPages, freeTop: 0, started: false };
  heap.allocate = (bytes) => {
    const chunk = Math.max(16, roundedUp(bytes + 4, 8));
    if (!(chunk < heap.freeTop)) {
      const grown = roundedUp(chunk + 48, PAGE_BYTES);
      heap.pages += grown / PAGE_BYTES;
      heap.freeTop += grown - (heap.started ? 0 : 40);
      heap.started = true;
    }
    heap.freeTop -= chunk;
  };
  return heap;
}

test("one block larger than the free top leaves more than one reserve free, and at most two reserves and two pages", async (context) => {
  const kernel = await bootKernel();
  const plain = memoryWithoutAReserve(kernel.pages());
  const allocate = (bytes) => {
    kernel.allocate(bytes);
    plain.allocate(bytes);
  };
  let smallBytes = 0;
  let pagesBefore = kernel.pages();
  let grew = false;
  while (!(grew && smallBytes >= 16 * MEBIBYTE)) {
    allocate(1024);
    smallBytes += 1024;
    grew = kernel.pages() !== pagesBefore;
    pagesBefore = kernel.pages();
  }
  const blockBytes = reserveBytes(kernel.pages()) + 2 * PAGE_BYTES;
  allocate(blockBytes);
  const reserve = reserveBytes(kernel.pages());
  const above = (kernel.pages() - plain.pages) * PAGE_BYTES;
  context.diagnostic(`${(smallBytes / MEBIBYTE).toFixed(2)} MiB of 1 KiB blocks, then one block of ${(blockBytes / MEBIBYTE).toFixed(2)} MiB: ${kernel.pages()} pages, ${plain.pages} without a reserve, ${(above / MEBIBYTE).toFixed(2)} MiB above = ${(above / reserve).toFixed(2)} reserves of ${(reserve / MEBIBYTE).toFixed(2)} MiB = ${((100 * above) / (plain.pages * PAGE_BYTES)).toFixed(1)} %`);
  assert.ok(above > reserve, `${above} bytes above, one reserve is ${reserve}`);
  assert.ok(above <= 2 * reserve + 2 * PAGE_BYTES, `${above} bytes above, two reserves and two pages are ${2 * reserve + 2 * PAGE_BYTES}`);
});
