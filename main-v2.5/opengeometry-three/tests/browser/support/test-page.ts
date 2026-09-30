import * as THREE from 'three';
import { OpenGeometry } from '../../../../dist/index.js';

export async function bootKernel(): Promise<URL> {
  const wasmURL = new URL('./opengeometry_bg.wasm', document.baseURI);
  const workerURL = new URL('./tessellation-worker.js', document.baseURI);
  const module = await WebAssembly.compileStreaming(fetch(wasmURL));
  const backend = new URLSearchParams(location.search).get('backend') === 'inline' ? 'inline' : 'worker';
  await OpenGeometry.create({ wasmModule: module }, { workerURL, tessellation: backend });
  return workerURL;
}

export function createRenderer(width: number, height: number): THREE.WebGLRenderer {
  const renderer = new THREE.WebGLRenderer({ antialias: false, preserveDrawingBuffer: true });
  renderer.setSize(width, height);
  document.body.append(renderer.domElement);
  return renderer;
}

export function releasePage(renderer: THREE.WebGLRenderer): void {
  OpenGeometry.reset();
  renderer.dispose();
  renderer.forceContextLoss();
  renderer.domElement.remove();
}

export function required<T>(value: T | null | undefined, label: string): T {
  if (value === null || value === undefined) throw new Error(`Test page expected ${label}`);
  return value;
}

export function statusElement(): Element {
  const status = document.querySelector('#status');
  if (!status) throw new Error('test page has no #status element');
  return status;
}

export function publishFixture(fixture: object, status: string): void {
  (window as typeof window & { __ogTest?: unknown }).__ogTest = fixture;
  statusElement().textContent = status;
}
