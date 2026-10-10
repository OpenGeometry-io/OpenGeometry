import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const PAGE_BYTES = 65536;
export const MEBIBYTE = 1024 * 1024;

const packageFolder = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../dist/opengeometry/pkg");

export function reserveBytes(pages) {
  return Math.min(Math.max(Math.floor(pages / 8) * PAGE_BYTES, MEBIBYTE), 64 * MEBIBYTE);
}

export async function bootKernel() {
  const wasmBytes = await readFile(path.join(packageFolder, "opengeometry_bg.wasm"));
  const glue = await import(pathToFileURL(path.join(packageFolder, "opengeometry.js")).href);
  let kernelExports;
  const instantiate = globalThis.WebAssembly.instantiate;
  globalThis.WebAssembly.instantiate = async (...parameters) => {
    const result = await instantiate(...parameters);
    kernelExports = (result.instance ?? result).exports;
    return result;
  };
  try {
    await glue.default({ module_or_path: wasmBytes });
  } finally {
    globalThis.WebAssembly.instantiate = instantiate;
  }
  const memory = kernelExports.memory;
  return {
    glue,
    wasmBytes,
    pages: () => memory.buffer.byteLength / PAGE_BYTES,
    allocate: (bytes) => kernelExports.__wbindgen_malloc(bytes, 8) >>> 0,
  };
}
