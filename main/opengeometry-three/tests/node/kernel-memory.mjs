import { readFileSync } from 'node:fs';
import init from '../../../dist/opengeometry/pkg/opengeometry.js';

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

export async function rawKernel() {
  const kernel = await init({ module_or_path: readKernelBytes() });
  const pages = () => kernel.memory.buffer.byteLength / PAGE_BYTES;
  return { pages, allocate: (bytes) => kernel.__wbindgen_malloc(bytes, 8) >>> 0 };
}
