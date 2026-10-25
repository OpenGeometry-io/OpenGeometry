import assert from 'node:assert/strict';
import { test } from 'node:test';
import { MEBIBYTE, PAGE_BYTES, rawKernel } from './kernel-memory.mjs';

function reserveBytes(pages) {
  return Math.min(Math.max(Math.floor(pages / 8) * PAGE_BYTES, MEBIBYTE), 64 * MEBIBYTE);
}

function roundedUp(value, step) {
  return Math.ceil(value / step) * step;
}

function memoryWithoutAReserve(startPages) {
  const heap = { pages: startPages, freeTop: 0, started: false };
  const allocate = (bytes) => {
    const chunk = Math.max(16, roundedUp(bytes + 4, 8));
    if (!(chunk < heap.freeTop)) {
      const grown = roundedUp(chunk + 48, PAGE_BYTES);
      heap.pages += grown / PAGE_BYTES;
      heap.freeTop += grown - (heap.started ? 0 : 40);
      heap.started = true;
    }
    heap.freeTop -= chunk;
  };
  return { heap, allocate };
}

const NAME = 'one block larger than the free top leaves more than one reserve free, '
  + 'and at most two reserves and two pages';

test(NAME, async () => {
  const kernel = await rawKernel();
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
  allocate(reserveBytes(kernel.pages()) + 2 * PAGE_BYTES);
  const reserve = reserveBytes(kernel.pages());
  const above = (kernel.pages() - plain.heap.pages) * PAGE_BYTES;
  process.stdout.write(`# ${kernel.pages()} pages, ${plain.heap.pages} without a reserve`);
  process.stdout.write(`, ${above / reserve} reserves above\n`);
  assert(above > reserve, `${above} bytes above, one reserve is ${reserve}`);
  const bound = 2 * reserve + 2 * PAGE_BYTES;
  assert(above <= bound, `${above} bytes above, two reserves and two pages are ${bound}`);
});
