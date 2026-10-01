import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { OpenGeometry, OGError, Solid, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_POLYLINE } from '../../../dist/index.js';
import { ensureGeometry, runtime, wanted } from '../../../dist/testing.js';

const MODULE = new WebAssembly.Module(readFileSync(new URL('../../../dist/opengeometry_bg.wasm', import.meta.url)));
await OpenGeometry.create({ wasmModule: MODULE });

function cube(ogId) {
  return new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId });
}

function failsWith(code, call) {
  return (error) => error instanceof OGError && error.code === code && error.call === call;
}

function listen(event) {
  const events = [];
  const stop = OpenGeometry.on(event, (detail) => { events.push(detail); });
  return { events, stop };
}

test('a real LimitExceeded from the inline backend gives a warning and a coarser record', () => {
  const body = cube('limit-cube');
  const warnings = listen('warning');
  const errors = listen('error');
  try {
    body.setAppearance({ deflection: 1.5 * body.displaySize().floor });
    ensureGeometry(body, true);
    assert.deepEqual(errors.events, []);
    assert.equal(warnings.events.length, 1);
    const warning = warnings.events[0];
    assert.equal(Object.getPrototypeOf(warning), Object.prototype);
    assert.equal(warning.code, 'LimitExceeded');
    assert.equal(warning.retryBucket, 2 * warning.bucket);
    assert.equal(body.record?.bucket, warning.retryBucket);
  } finally {
    warnings.stop();
    errors.stop();
    body.dispose();
  }
});

test('a stale handle error names the public method', () => {
  const body = cube('stale-cube');
  body.dispose();
  assert.throws(() => body.getBounds(), failsWith('Disposed', 'Solid.getBounds'));
  assert.throws(() => body.getReport(), failsWith('Disposed', 'Solid.getReport'));
  const points = [[0, 0, 0], [1, 0, 0], [1, 1, 0]];
  assert.throws(() => new Solid(OG_PRIMITIVE_POLYLINE, { points }, { ogId: 'solid-polyline' }), (error) =>
    error instanceof OGError && error.call === 'Solid.constructor');
});

test('a malformed display buffer is reported as an OGError', () => {
  const body = cube('malformed-buffer-cube');
  const provider = Object(runtime().provider);
  const errors = listen('error');
  provider.compute = () => ({});
  try {
    ensureGeometry(body, true);
    assert.equal(errors.events.length, 1);
    assert(errors.events[0] instanceof OGError);
    assert.equal(errors.events[0].code, 'WorkerFailure');
  } finally {
    delete provider.compute;
    errors.stop();
    body.dispose();
  }
});

test('a malformed kernel payload is rejected by the decoder', () => {
  const body = cube('malformed-brep-cube');
  const graph = Object(runtime().graph);
  graph.brep = () => '{}';
  try {
    assert.throws(() => body.getBrep(), failsWith('InvalidGeometry', 'Solid.getBrep'));
  } finally {
    delete graph.brep;
    body.dispose();
  }
  const before = runtime().bodies.size;
  graph.node = () => '{}';
  try {
    assert.throws(() => cube('malformed-node-cube'), failsWith('InvalidGeometry', 'Solid.constructor'));
  } finally {
    delete graph.node;
  }
  assert.equal(runtime().bodies.size, before);
});

test('an invalid deflection is an OGError', () => {
  const body = cube('nan-deflection-cube');
  try {
    body.setAppearance({ deflection: Number.NaN });
    assert.throws(() => wanted(body), (error) => error instanceof OGError && error.code === 'InvalidParameter');
  } finally {
    body.dispose();
  }
});

test('a stubbed RuntimeError poisons the runtime emits fatal and names the method', () => {
  const body = cube('panic-cube');
  const graph = Object(runtime().graph);
  const fatal = listen('fatal');
  runtime().displayed.add(body);
  graph.buffers = () => { throw new WebAssembly.RuntimeError('unreachable'); };
  Reflect.set(globalThis, '__opengeometryPanic', 'index out of bounds');
  try {
    try {
      OpenGeometry.flush({ geometry: 'sync' });
    } catch (error) {
      assert(error instanceof OGError && error.code === 'KernelPanic');
    }
    assert.equal(fatal.events.length, 1);
    const [panic] = fatal.events;
    assert(panic instanceof OGError);
    assert.equal(panic.code, 'KernelPanic');
    assert.equal(panic.call, 'tessellation');
    assert.equal(panic.message, 'index out of bounds');
    assert.throws(() => cube('after-panic-cube'), (error) =>
      error instanceof OGError && error.code === 'KernelPanic' && error.message === 'kernel runtime is poisoned');
  } finally {
    delete graph.buffers;
    Reflect.deleteProperty(globalThis, '__opengeometryPanic');
    fatal.stop();
    OpenGeometry.reset();
  }
});
