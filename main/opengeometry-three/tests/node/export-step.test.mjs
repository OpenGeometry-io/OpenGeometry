import assert from 'node:assert/strict';
import { test } from 'node:test';
import { OpenGeometry, SystemAssembly } from '../../../dist/index.js';
import { boot, cuboid, failsWith } from './support.mjs';

function rejection(promise) {
  return promise.then(() => undefined, (reason) => reason);
}

test('exportStep on a disposed body whose ogId was reused rejects with Disposed', async () => {
  await boot();
  const stale = cuboid('reuse-1');
  stale.dispose();
  cuboid('reuse-1');
  assert(failsWith('Disposed')(await rejection(OpenGeometry.exportStep({ nodes: [stale] }))));
});

test('every handle is checked in order', async () => {
  await boot();
  const live = cuboid('export-live');
  const stale = cuboid('reuse-2');
  stale.dispose();
  cuboid('reuse-2');
  assert(failsWith('Disposed')(await rejection(OpenGeometry.exportStep({ nodes: [live, stale] }))));
});

test('a disposed assembly rejects with Disposed, not UnknownNode', async () => {
  await boot();
  const assembly = new SystemAssembly({ ogId: 'export-assembly' });
  assembly.dispose();
  assert(failsWith('Disposed')(await rejection(OpenGeometry.exportStep({ nodes: [assembly] }))));
});

test('a body from a reset runtime rejects with Disposed', async () => {
  await boot();
  const stale = cuboid('reset-export');
  OpenGeometry.reset();
  await boot();
  cuboid('reset-export');
  assert(failsWith('Disposed')(await rejection(OpenGeometry.exportStep({ nodes: [stale] }))));
});
