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

export function takePanicMessage(): string | undefined {
  const message: unknown = Reflect.get(globalThis, '__opengeometryPanic');
  Reflect.deleteProperty(globalThis, '__opengeometryPanic');
  return typeof message === 'string' ? message : undefined;
}
