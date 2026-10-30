import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CYLINDER } from '../../../dist/index.js';
import { boot, cuboid, nextTask } from './support.mjs';

const ROUNDS = 5;
const ROUND = 200;

function cylinder() {
  return new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.5, height: 1 }, { appearance: { outline: false } });
}

function median(values) {
  return [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
}

function perBody() {
  const rounds = [];
  for (let round = 0; round < ROUNDS; round++) {
    const start = performance.now();
    const bodies = Array.from({ length: ROUND }, cylinder);
    for (const body of bodies) body.dispose();
    rounds.push((performance.now() - start) / ROUND);
  }
  return median(rounds);
}

function shown(body) {
  return {
    record: body.record,
    surface: body.surface.geometry,
    outline: body.outline.geometry,
    visible: [body.visible, body.surface.visible, body.outline.visible],
    matrix: body.matrixWorld.toArray(),
  };
}

test('a body create and dispose costs at most 3x more with 2,000 live bodies than with none', async () => {
  await boot();
  await nextTask();
  const empty = perBody();
  const live = Array.from({ length: 2_000 }, cylinder);
  try {
    const full = perBody();
    const message = `${full.toFixed(3)} ms per body with 2,000 live bodies, ${empty.toFixed(3)} ms with none`;
    assert(full <= 3 * empty, message);
  } finally {
    for (const body of live) body.dispose();
  }
});

test('disposing a body leaves every other body geometry and display unchanged', async () => {
  await boot();
  await nextTask();
  const bodies = Array.from({ length: 20 }, (_, index) => cuboid(`kept-${String(index)}`, 1 + index / 10));
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  await nextTask();
  const [gone, ...kept] = bodies;
  const before = kept.map(shown);
  assert(before.every((entry) => entry.record !== undefined));
  gone?.dispose();
  OpenGeometry.flush();
  await nextTask();
  try {
    for (const [index, entry] of kept.map(shown).entries()) {
      const previous = before[index];
      assert.equal(entry.record, previous?.record);
      assert.equal(entry.surface, previous?.surface);
      assert.equal(entry.outline, previous?.outline);
      assert.deepEqual([entry.visible, entry.matrix], [previous?.visible, previous?.matrix]);
    }
    assert.equal(gone?.visible, false);
  } finally {
    for (const body of kept) body.dispose();
  }
});
