import initializeKernel, {
  initSync, OGTessellator, OGWorldGraph, type InitOutput,
} from '../../../opengeometry/pkg/opengeometry.js';

export { initializeKernel, initSync, OGTessellator, OGWorldGraph };

export function compileKernel(wasmURL: string | URL): Promise<WebAssembly.Module> {
  return WebAssembly.compileStreaming(fetch(wasmURL));
}

export function initKernel(module: WebAssembly.Module): Promise<InitOutput> {
  return initializeKernel({ module_or_path: module });
}
