import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID } from '../../../dist/index.js';
import { boot } from './support.mjs';

async function fresh() {
  await boot();
  OpenGeometry.reset();
}

function placedBox() {
  return new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 3 }, {
    plane: { origin: [2, -1, 0.5], normal: [0, 0, 1], xDirection: [1, 0, 0] },
  });
}

function childUpdatesDuring(body, step) {
  const update = THREE.Object3D.prototype.updateMatrix;
  const children = new Set([body.surface, body.outline]);
  let calls = 0;
  THREE.Object3D.prototype.updateMatrix = function counted(...rest) {
    if (children.has(this)) calls++;
    return Reflect.apply(update, this, rest);
  };
  try {
    step();
  } finally {
    THREE.Object3D.prototype.updateMatrix = update;
  }
  return calls;
}

function installed(body) {
  const scene = new THREE.Scene();
  scene.add(body);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  const record = body.record;
  assert(record, 'the body has an installed record');
  return record;
}

test('a forced world walk from a body updates no local matrix of its surface or outline', async () => {
  await fresh();
  const body = placedBox();
  installed(body);
  assert.equal(childUpdatesDuring(body, () => { body.updateMatrixWorld(true); }), 0);
});

test('an installed record places the surface and outline at the record origin in world space', async () => {
  await fresh();
  const body = placedBox();
  const record = installed(body);
  assert.notEqual(record.origin.length(), 0);
  const expected = record.origin.clone().applyMatrix4(body.matrixWorld);
  for (const child of [body.surface, body.outline]) {
    const actual = new THREE.Vector3().setFromMatrixPosition(child.matrixWorld);
    assert(actual.distanceTo(expected) < 1e-9, `${child.type} at ${actual.toArray().join(',')}`);
  }
  body.updateMatrixWorld(true);
  const walked = new THREE.Vector3().setFromMatrixPosition(body.surface.matrixWorld);
  assert(walked.distanceTo(expected) < 1e-9);
});
