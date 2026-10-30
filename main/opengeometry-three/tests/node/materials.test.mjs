import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry, Wire, OG_PRIMITIVE_RECTANGLE } from '../../../dist/index.js';
import { runtime } from '../../../dist/testing.js';
import { boot, cuboid, failsWith } from './support.mjs';

function rectangle(ogId, appearance = {}) {
  return new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId, appearance });
}

function lineMaterialOf(body) {
  const material = body.outline.material;
  assert(material instanceof THREE.LineBasicMaterial);
  return material;
}

test("a wire's colour and opacity follow setAppearance", async () => {
  await boot();
  const wire = rectangle('appearance-wire');
  try {
    wire.setAppearance({ color: 0xff0000, opacity: 0.5 });
    const material = lineMaterialOf(wire);
    assert.equal(material.color.getHex(), 0xff0000);
    assert.equal(material.opacity, 0.5);
    assert.equal(material.transparent, true);
  } finally {
    wire.dispose();
  }
});

test('surface materials are shaded and line materials are pooled by colour and opacity', async () => {
  await boot();
  const first = rectangle('pooled-wire-1', { color: 0x00ff00 });
  const second = rectangle('pooled-wire-2', { color: 0x00ff00 });
  const solid = cuboid('shaded-solid');
  try {
    assert.equal(lineMaterialOf(first), lineMaterialOf(second));
    assert.notEqual(lineMaterialOf(solid), lineMaterialOf(first));
    const surface = solid.surface.material;
    assert(surface instanceof THREE.MeshStandardMaterial);
    assert.equal(surface.polygonOffset, true);
  } finally {
    first.dispose();
    second.dispose();
    solid.dispose();
  }
});

test('setAppearance on a body from a reset runtime throws Disposed and leaves the new pool alone', async () => {
  await boot();
  const stale = cuboid('stale-appearance');
  OpenGeometry.reset();
  await boot();
  const fresh = cuboid('fresh-appearance');
  try {
    assert.throws(() => { stale.setAppearance({ color: 0x123456 }); }, failsWith('Disposed'));
    const entry = runtime().surfacePool.get(`${String(0x6699dd)}:1`);
    assert(entry);
    assert.equal(entry.refs, 1);
    assert.equal(entry.material, fresh.surface.material);
  } finally {
    fresh.dispose();
  }
});
