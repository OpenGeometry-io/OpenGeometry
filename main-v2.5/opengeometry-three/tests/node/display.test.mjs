import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OG_TRANSFORM_TRANSLATE } from '../../../dist/index.js';
import { graph } from '../../../dist/testing.js';
import { boot, cuboid, listen, nextTask } from './support.mjs';

function assertNear(actual, expected, tolerance) {
  assert.equal(actual.length, expected.length);
  actual.forEach((value, index) => {
    assert(Math.abs(value - (expected[index] ?? Number.NaN)) <= tolerance, `${String(value)} at ${String(index)}`);
  });
}

test('a body without a record is visible, culled and bounded by the kernel local bounds', async () => {
  await boot();
  await nextTask();
  const body = cuboid('placeholder-cube', 2);
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [5, 0, 0] });
  const copy = body.instance({ ogId: 'placeholder-copy' });
  try {
    assert.equal(body.surface.visible, true);
    assert.equal(body.surface.frustumCulled, true);
    const box = body.surface.geometry.boundingBox;
    assert(box);
    assertNear([...box.min.toArray(), ...box.max.toArray()], [-1, 0, -0.5, 1, 1, 0.5], 1e-7);
    assert.equal(copy.surface.geometry, body.surface.geometry);
  } finally {
    copy.dispose();
    body.dispose();
  }
});

test("writing a body's position emits a placement warning at the next matrix update", async () => {
  await boot();
  await nextTask();
  const body = cuboid('overwritten-cube');
  const scene = new THREE.Scene();
  scene.add(body);
  const warnings = listen('warning');
  try {
    body.position.set(1, 2, 3);
    scene.updateMatrixWorld(true);
    assert.deepEqual(warnings.events, [{ code: 'PlacementOverwritten', ogId: body.ogId }]);
    assert.deepEqual(body.position.toArray(), [0, 0, 0]);
  } finally {
    warnings.stop();
    body.dispose();
  }
});

test('a Three parent that moves after placement keeps the body at the kernel world matrix', async () => {
  await boot();
  await nextTask();
  const body = cuboid('moved-parent-cube');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 2, 3] });
  const scene = new THREE.Scene();
  const group = new THREE.Group();
  scene.add(group);
  group.add(body);
  const warnings = listen('warning');
  try {
    scene.updateMatrixWorld(true);
    group.position.set(3, 0, 0);
    group.rotation.set(0, 0.5, 0);
    scene.updateMatrixWorld(true);
    assertNear(body.matrixWorld.elements, Array.from(graph().worldMatrix(body.ogId)), 1e-12);
    const local = group.matrixWorld.clone().invert().multiply(body.matrixWorld);
    assertNear(body.matrix.elements, local.elements, 1e-12);
    assert.deepEqual(warnings.events, []);
  } finally {
    warnings.stop();
    body.dispose();
  }
});
