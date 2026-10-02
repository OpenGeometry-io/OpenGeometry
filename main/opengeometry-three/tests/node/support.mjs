import { readFileSync } from 'node:fs';
import { OpenGeometry, OGError, Solid, OG_PRIMITIVE_CUBOID } from '../../../dist/index.js';

const MODULE = new WebAssembly.Module(readFileSync(new URL('../../../dist/opengeometry_bg.wasm', import.meta.url)));

export function boot() {
  return OpenGeometry.create({ wasmModule: MODULE });
}

export function nextTask() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

export function cuboid(ogId, width = 1) {
  return new Solid(OG_PRIMITIVE_CUBOID, { width, height: 1, depth: 1 }, { ogId });
}

export function failsWith(code) {
  return (error) => error instanceof OGError && error.code === code;
}

export function listen(event) {
  const events = [];
  const stop = OpenGeometry.on(event, (detail) => { events.push(detail); });
  return { events, stop };
}
