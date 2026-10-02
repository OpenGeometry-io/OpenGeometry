import { OpenGeometry } from '../../../dist/index.js';

export async function bootExample(base: string): Promise<void> {
  const wasmURL = new URL(`${base}opengeometry_bg.wasm`, document.baseURI);
  const workerURL = new URL(`${base}tessellation-worker.js`, document.baseURI);
  const module = await WebAssembly.compileStreaming(fetch(wasmURL));
  const backend = new URLSearchParams(location.search).get('backend') === 'inline' ? 'inline' : 'worker';
  await OpenGeometry.create({ wasmModule: module }, { workerURL, tessellation: backend });
}
