import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, OG_OPERATION_SHELL, OG_OPERATION_SUBTRACT, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { graph, runtime } from '../../../dist/testing.js';
import { boot, cuboid, nextTask } from './support.mjs';

function cuboids(prefix) {
  return Array.from({ length: 50 }, (_, index) => cuboid(`${prefix}-${String(index)}`, 1 + index / 100));
}

function disposeAll(bodies) {
  for (const body of bodies) body.dispose();
}

test('flush after a transform makes no node call', async () => {
  await boot();
  await nextTask();
  const bodies = cuboids('node-call');
  OpenGeometry.flush();
  const target = Object(graph());
  const node = target.node;
  let calls = 0;
  target.node = (...args) => { calls++; return Reflect.apply(node, target, args); };
  try {
    bodies[0]?.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    OpenGeometry.flush();
  } finally {
    delete target.node;
    disposeAll(bodies);
  }
  assert.equal(calls, 0);
});

test('flush purges records only for the shapes in the change set', async () => {
  await boot();
  await nextTask();
  const bodies = cuboids('purge');
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  await nextTask();
  const records = Object(runtime().records);
  const purge = records.purge;
  let calls = 0;
  records.purge = (...args) => { calls++; return Reflect.apply(purge, records, args); };
  try {
    bodies[0]?.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    OpenGeometry.flush();
  } finally {
    delete records.purge;
    disposeAll(bodies);
  }
  assert(calls <= 1, `${String(calls)} purge calls`);
});

async function shown(prefix) {
  const bodies = cuboids(prefix);
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  await nextTask();
  return bodies;
}

function swapsDuring(step) {
  const prototype = Object.getPrototypeOf(Solid.prototype);
  const swap = prototype.swapReadyRecord;
  let calls = 0;
  prototype.swapReadyRecord = function counted(...rest) { calls++; return Reflect.apply(swap, this, rest); };
  try {
    step();
  } finally {
    prototype.swapReadyRecord = swap;
  }
  return calls;
}

test('operate flushes with scope changed: only the operated bodies swap', async () => {
  await boot();
  await nextTask();
  const bodies = await shown('operate');
  const [host, tool, shelled] = bodies;
  assert(host && tool && shelled);
  try {
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [0.5, 0, 0] });
    const cut = swapsDuring(() => host.operate(OG_OPERATION_SUBTRACT, { tools: [tool] }));
    const shell = swapsDuring(() => shelled.operate(OG_OPERATION_SHELL, { thickness: 0.1 }));
    assert(cut <= 2, `${String(cut)} swaps for one subtract with ${String(bodies.length)} displayed bodies`);
    assert(shell <= 1, `${String(shell)} swaps for one shell with ${String(bodies.length)} displayed bodies`);
  } finally {
    disposeAll(bodies);
  }
});
