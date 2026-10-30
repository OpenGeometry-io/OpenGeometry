import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, OGError } from '../../../dist/index.js';
import { boot, cuboid, failsWith, nextTask } from './support.mjs';

test('rollback revives the old object after its ogId was reused under a mark', async () => {
  await boot();
  await nextTask();
  const scene = new THREE.Scene();
  const first = cuboid('reused-id');
  scene.add(first);
  scene.updateMatrixWorld(true);
  const mark = OpenGeometry.mark();
  first.dispose();
  const second = cuboid('reused-id', 2);
  scene.add(second);
  mark.rollback();
  mark.release();
  await nextTask();
  try {
    assert.equal(first.parent, scene);
    assert.equal(first.visible, true);
    assert.equal(first.inLimbo, false);
    assert.notEqual(first.getBounds(), null);
    assert.equal(second.parent, null);
    assert.equal(second.inLimbo, false);
    assert.throws(() => second.getBounds(), failsWith('Disposed'));
  } finally {
    first.dispose();
  }
});

test('a mark truncated by an earlier rollback is released in the SDK and later limbo bodies finalise', async () => {
  await boot();
  await nextTask();
  const first = OpenGeometry.mark();
  const second = OpenGeometry.mark();
  first.rollback();
  first.release();
  const body = cuboid('after-truncation');
  body.dispose();
  assert.equal(body.inLimbo, false);
  assert.throws(() => { second.release(); }, (error) =>
    error instanceof OGError && error.code === 'InvalidMark' && error.message === 'mark is released');
});

test('a limbo body is finalised when no live mark older than its dispose remains', async () => {
  await boot();
  await nextTask();
  const body = cuboid('older-mark-body');
  const first = OpenGeometry.mark();
  body.dispose();
  const second = OpenGeometry.mark();
  try {
    first.release();
    assert.equal(body.inLimbo, false);
  } finally {
    second.release();
  }
});
