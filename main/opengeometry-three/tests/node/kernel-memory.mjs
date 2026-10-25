import { readFileSync } from 'node:fs';

export const PAGE_BYTES = 65536;
export const MEBIBYTE = 1024 * 1024;

export function readKernelBytes() {
  return readFileSync(new URL('../../../dist/opengeometry/pkg/opengeometry_bg.wasm', import.meta.url));
}

export async function bootKernel(start) {
  const captured = { memory: new WebAssembly.Memory({ initial: 0 }) };
  const instantiate = globalThis.WebAssembly.instantiate;
  Reflect.set(globalThis.WebAssembly, 'instantiate', async (source, imports) => {
    const result = await instantiate(source, imports);
    const memory = Reflect.get((result.instance ?? result).exports, 'memory');
    if (memory instanceof WebAssembly.Memory) captured.memory = memory;
    return result;
  });
  try {
    await start();
  } finally {
    globalThis.WebAssembly.instantiate = instantiate;
  }
  return { pages: () => captured.memory.buffer.byteLength / PAGE_BYTES };
}
