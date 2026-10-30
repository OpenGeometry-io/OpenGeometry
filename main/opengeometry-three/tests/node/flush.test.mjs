import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, OG_TRANSFORM_TRANSLATE } from '../../../dist/index.js';
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
