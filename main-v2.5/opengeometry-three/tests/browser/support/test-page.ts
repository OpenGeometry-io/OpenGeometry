import * as THREE from 'three';
import { OpenGeometry } from '../../../../dist/index.js';

const ERRORS: unknown[] = [];

export async function bootKernel(): Promise<URL> {
  const params = new URLSearchParams(location.search);
  const wasmURL = new URL('./opengeometry_bg.wasm', document.baseURI);
  const workerURL = new URL(workerPath(params.get('worker')), document.baseURI);
  const module = await WebAssembly.compileStreaming(fetch(wasmURL));
  const backend = params.get('backend') === 'inline' ? 'inline' : 'worker';
  await OpenGeometry.create({ wasmModule: module }, { workerURL, tessellation: backend });
  OpenGeometry.on('error', (event) => { ERRORS.push(event); });
  return workerURL;
}

function workerPath(name: string | null): string {
  if (name === 'missing') return './missing-worker.js';
  if (name === 'crash') return './crash-worker.ts';
  return './tessellation-worker.js';
}

export function recordedErrors(): unknown[] {
  return [...ERRORS];
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

export function publishFixture<K extends 'ogSmoke' | 'ogAcceptance'>(
  key: K, fixture: NonNullable<(typeof globalThis)[K]>, status: string,
): void {
  globalThis[key] = fixture;
  statusElement().textContent = status;
}
