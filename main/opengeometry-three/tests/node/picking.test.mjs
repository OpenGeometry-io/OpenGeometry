import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry } from '../../../dist/index.js';
import { runtime } from '../../../dist/testing.js';
import { boot, cuboid } from './support.mjs';

function linearFaceId(ranges, faceIndex) {
  for (let offset = 0; offset < ranges.length; offset += 3) {
    if (faceIndex >= ranges[offset + 1] && faceIndex < ranges[offset + 1] + ranges[offset + 2]) return ranges[offset];
  }
  return undefined;
}

function hitOn(object, fields) {
  return { distance: 0, point: new THREE.Vector3(), object, ...fields };
}

function displayed(bodies) {
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  for (const body of bodies) assert(body.record, `${body.ogId} has a record`);
}

function countedRanges(faces) {
  const ranges = new Uint32Array(faces * 3);
  let start = 0;
  for (let face = 0; face < faces; face++) {
    const count = 1 + (face % 3);
    ranges.set([(face * 7919) % faces, start, count], face * 3);
    start += count;
  }
  const counter = { reads: 0, triangles: start };
  const proxy = new Proxy(ranges, {
    get(target, property) {
      counter.reads++;
      return Reflect.get(target, property);
    },
  });
  return { ranges, proxy, counter };
}

test('resolveHit maps a hit object to its body without scanning the body map', async () => {
  await boot();
  const middle = cuboid('pick-b', 2);
  const solids = [cuboid('pick-a', 1), middle, cuboid('pick-c', 3)];
  displayed(solids);
  const record = middle.record;
  assert(record);
  const bodies = Object(runtime().bodies);
  bodies.values = () => { throw new Error('resolveHit scanned the body map'); };
  try {
    assert.deepEqual(OpenGeometry.resolveHit(hitOn(middle.surface, { faceIndex: 0 })), {
      ogId: 'pick-b', shapeRevision: record.revision, faceId: linearFaceId(record.faceRanges, 0),
    });
  } finally {
    delete bodies.values;
  }
  for (const solid of solids) solid.dispose();
});

test('resolveHit finds the face by binary search over the face ranges', async () => {
  await boot();
  const body = cuboid('pick-search');
  const faces = 4096;
  const { ranges, proxy, counter } = countedRanges(faces);
  const target = Object(body);
  const kept = target.record;
  target.record = { revision: 7, faceRanges: proxy, edgeIds: new Uint32Array(0) };
  const pick = (faceIndex) => OpenGeometry.resolveHit(hitOn(body.surface, { faceIndex }))?.faceId;
  try {
    const lastStart = ranges[faces * 3 - 2];
    counter.reads = 0;
    assert.equal(pick(lastStart), linearFaceId(ranges, lastStart));
    const limit = 3 * (Math.log2(faces) + 2);
    assert(counter.reads <= limit, `${String(counter.reads)} reads for the last face, limit ${String(limit)}`);
    for (let offset = 0; offset < ranges.length; offset += 3) {
      const first = Number(ranges[offset + 1]);
      const last = first + Number(ranges[offset + 2]) - 1;
      assert.equal(pick(first), linearFaceId(ranges, first));
      assert.equal(pick(last), linearFaceId(ranges, last));
    }
    assert.equal(pick(counter.triangles), undefined);
  } finally {
    target.record = kept;
  }
  body.dispose();
});

test("a hit on a disposed body's mesh resolves nothing", async () => {
  await boot();
  const body = cuboid('pick-disposed');
  displayed([body]);
  const { surface, outline } = body;
  body.dispose();
  assert.equal(OpenGeometry.resolveHit(hitOn(surface, { faceIndex: 0 })), undefined);
  assert.equal(OpenGeometry.resolveHit(hitOn(outline, { index: 0 })), undefined);
});
