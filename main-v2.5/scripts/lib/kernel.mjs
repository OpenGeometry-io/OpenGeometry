import { readFileSync } from 'node:fs';
import { OpenGeometry } from '../../dist/index.js';

export async function createInlineKernel() {
  const bytes = readFileSync(new URL('../../dist/opengeometry_bg.wasm', import.meta.url));
  await OpenGeometry.create({ wasmModule: new WebAssembly.Module(bytes) }, { tessellation: 'inline' });
}
