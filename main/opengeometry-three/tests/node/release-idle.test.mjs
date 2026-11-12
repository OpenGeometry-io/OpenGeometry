import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import { OpenGeometry } from '../../../dist/index.js';
import * as TESTING from '../../../dist/testing.js';
import { boot, cuboid } from './support.mjs';

async function fresh() {
  await boot();
  OpenGeometry.reset();
}

function pool() {
  return Object(TESTING.runtime().records);
}

function idleRecords() {
  return [...Reflect.get(pool(), 'idle')];
}

function disposals(records) {
  const disposed = new Set();
  for (const record of records) {
    record.outline.addEventListener('dispose', () => { disposed.add(record); });
  }
  return disposed;
}

async function shownWithIdle(ogId) {
  const body = cuboid(ogId);
  const scene = new THREE.Scene();
  scene.add(body);
  scene.updateMatrixWorld(true);
  await OpenGeometry.settled();
  const coarse = body.record;
  body.setAppearance({ deflection: 2 ** -12 });
  await OpenGeometry.settled();
  return { body, coarse };
}

test('idle records are disposed and their count and bytes returned', async () => {
  await fresh();
  const { body, coarse } = await shownWithIdle('idle-cube');
  try {
    const idle = idleRecords();
    assert.deepEqual(idle, [coarse]);
    const disposed = disposals(idle);
    const bytes = Reflect.get(pool(), 'idleBytes');
    assert.equal(bytes, coarse?.bytes);
    assert.deepEqual(OpenGeometry.releaseIdleGeometry(), { records: 1, bytes });
    assert.deepEqual([...disposed], idle);
    assert.deepEqual(idleRecords(), []);
    assert.equal(Reflect.get(pool(), 'idleBytes'), 0);
    assert.deepEqual(OpenGeometry.releaseIdleGeometry(), { records: 0, bytes: 0 });
  } finally {
    body.dispose();
  }
});

test('installed records survive a release', async () => {
  await fresh();
  const { body } = await shownWithIdle('installed-cube');
  try {
    const installed = body.record;
    if (!installed) throw new Error('no installed record');
    const disposed = disposals([installed]);
    OpenGeometry.releaseIdleGeometry();
    assert.equal(body.record, installed);
    assert.equal(disposed.size, 0);
    assert.equal(pool().get(installed.shapeId, installed.revision, installed.bucket), installed);
  } finally {
    body.dispose();
  }
});

test('a later ensureGeometry re-requests a released bucket', async () => {
  await fresh();
  const { body, coarse } = await shownWithIdle('rerequest-cube');
  const provider = Object(TESTING.runtime().provider);
  const compute = provider.compute.bind(provider);
  const requests = [];
  provider.compute = (request) => { requests.push(request.bucket); return compute(request); };
  try {
    OpenGeometry.releaseIdleGeometry();
    body.setAppearance({ deflection: coarse?.bucket ?? 1 });
    TESTING.ensureGeometry(body, true);
    assert.deepEqual(requests, [coarse?.bucket]);
    assert.notEqual(body.record, coarse);
    assert.equal(body.record?.bucket, coarse?.bucket);
  } finally {
    delete provider.compute;
    body.dispose();
  }
});

test('a release after a reset returns nothing released', async () => {
  await fresh();
  await shownWithIdle('reset-cube');
  OpenGeometry.reset();
  assert.deepEqual(OpenGeometry.releaseIdleGeometry(), { records: 0, bytes: 0 });
});
