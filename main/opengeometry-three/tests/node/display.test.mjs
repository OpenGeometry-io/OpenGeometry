import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_EXTRUDE, OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_PROFILE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { graph } from '../../../dist/testing.js';
import { boot, cuboid, listen, nextTask } from './support.mjs';

function assertNear(actual, expected, tolerance) {
  assert.equal(actual.length, expected.length);
  actual.forEach((value, index) => {
    assert(Math.abs(value - (expected[index] ?? Number.NaN)) <= tolerance, `${String(value)} at ${String(index)}`);
  });
}

function capSegments(body, bucket) {
  const { shapeId } = JSON.parse(graph().node(body.ogId));
  const { edgeIds } = graph().buffers(shapeId, bucket, 2_000_000);
  const counts = new Map();
  for (const id of edgeIds) counts.set(id, (counts.get(id) ?? 0) + 1);
  return [...counts.values()].filter((count) => count > 1);
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

test('a thin cylinder keeps an even count of twelve or more cap segments at rest and in plan view', async () => {
  await boot();
  await nextTask();
  const body = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.011, height: 0.45 }, { ogId: 'thin-leg' });
  try {
    const { static: bucket } = JSON.parse(graph().displayBuckets(body.ogId));
    assert.deepEqual(capSegments(body, bucket), [18, 18]);
    assert.deepEqual(capSegments(body, 2 ** -7), [12, 12]);
  } finally {
    body.dispose();
  }
});

function prism(ogId, appearance) {
  const corner = (i) => [2 * Math.cos(i * Math.PI / 13), 2 * Math.sin(i * Math.PI / 13)];
  const segments = Array.from({ length: 26 }, (_, i) => ({ to: corner(i + 1) }));
  const profile = { start: corner(0), segments, closed: true };
  const outline = new Wire(OG_PRIMITIVE_PROFILE, profile, { ogId: `${ogId}-outline` });
  const body = new Solid(OG_OPERATION_EXTRUDE, { profile: outline, distance: 3 }, { ogId, appearance });
  return { outline, body };
}

function drawn(buffers) {
  return new Set(buffers.edgeIds).size;
}

function smooth(buffers) {
  return buffers.smoothEdges.reduce((sum, flag) => sum + flag, 0);
}

test('a polygon prism hides its folds under the smooth angle and flags them per edge', async () => {
  await boot();
  const { outline, body } = prism('prism');
  try {
    const { shapeId } = JSON.parse(graph().node(body.ogId));
    const { static: bucket } = JSON.parse(graph().displayBuckets(body.ogId));
    const folded = graph().buffers(shapeId, bucket, 2_000_000);
    const sharp = graph().buffers(shapeId, bucket, 2_000_000, 0);
    assert.deepEqual([drawn(folded), smooth(folded), folded.smoothEdges.length], [52, 26, 78]);
    assert.deepEqual([drawn(sharp), smooth(sharp)], [78, 0]);
    assert.throws(() => graph().buffers(shapeId, bucket, 2_000_000, 90));
  } finally {
    body.dispose();
    outline.dispose();
  }
});

test('a body smooth angle reaches its rendered outline', async () => {
  await boot();
  const folded = prism('folded-prism');
  const sharp = prism('sharp-prism', { smoothAngle: 0 });
  const scene = new THREE.Scene();
  scene.add(folded.body, sharp.body);
  try {
    scene.updateMatrixWorld(true);
    OpenGeometry.flush({ geometry: 'sync' });
    assert.deepEqual([drawn(folded.body.record), smooth(folded.body.record)], [52, 26]);
    assert.deepEqual([drawn(sharp.body.record), smooth(sharp.body.record)], [78, 0]);
  } finally {
    for (const { body, outline } of [folded, sharp]) {
      body.dispose();
      outline.dispose();
    }
  }
});
