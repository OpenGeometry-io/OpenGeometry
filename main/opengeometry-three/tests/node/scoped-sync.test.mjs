import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, SystemAssembly, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { graph, runtime } from '../../../dist/testing.js';
import { boot, failsWith, nextTask } from './support.mjs';

const BODY = Object.getPrototypeOf(Solid.prototype);

async function fresh() {
  await boot();
  OpenGeometry.reset();
}

function box(width, x = 0) {
  return new Solid(OG_PRIMITIVE_CUBOID, { width, height: 1, depth: 1 }, {
    plane: { origin: [x, 0, 0], normal: [0, 0, 1], xDirection: [1, 0, 0] },
  });
}

function pendingWorker() {
  const provider = Object(runtime().provider);
  const cancelled = [];
  const pending = [];
  provider.compute = undefined;
  provider.request = () => new Promise((resolve) => { pending.push(resolve); });
  provider.cancelShape = (shapeId) => { cancelled.push(shapeId); };
  return { cancelled, pending };
}

function tessellationLog() {
  const target = Object(graph());
  const buffers = target.buffers;
  const log = { shapes: [''].slice(1), stop: () => { delete target.buffers; } };
  target.buffers = (shapeId, ...rest) => {
    log.shapes.push(shapeId);
    return Reflect.apply(buffers, target, [shapeId, ...rest]);
  };
  return log;
}

function swapLog() {
  const swap = BODY.swapReadyRecord;
  const log = { bodies: new Set(), stop: () => { BODY.swapReadyRecord = swap; } };
  BODY.swapReadyRecord = function counted(...rest) {
    log.bodies.add(this);
    return Reflect.apply(swap, this, rest);
  };
  return log;
}

function current(body) {
  return body.record?.shapeId === body.lastInfo.shapeId && body.record?.revision === body.lastInfo.shapeRevision;
}

function placed(body, mesh) {
  const record = body.record;
  assert(record, `${body.ogId} has a record`);
  const expected = body.matrixWorld.clone().multiply(new THREE.Matrix4().makeTranslation(record.origin));
  return mesh.matrixWorld.elements.every((value, index) => Math.abs(value - (expected.elements[index] ?? 0)) < 1e-9);
}

function scopedSync(bodies) {
  const log = tessellationLog();
  const swaps = swapLog();
  try {
    Reflect.apply(OpenGeometry.flush, OpenGeometry, [{ geometry: 'sync', bodies }]);
  } finally {
    log.stop();
    swaps.stop();
  }
  return { shapes: log.shapes, swapped: swaps.bodies };
}

async function pendingScene(count) {
  const worker = pendingWorker();
  const bodies = Array.from({ length: count }, (_, index) => box(1 + (index % 997) / 1000, 2 * index));
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  await nextTask();
  assert(bodies.every((body) => body.record === undefined), 'every displayed body waits for its record');
  return { worker, bodies, scene };
}

for (const count of [1_000, 4_000]) {
  test(`a scoped sync flush meshes only the given body among ${String(count)} pending bodies`, async () => {
    await fresh();
    const { worker, bodies } = await pendingScene(count);
    const [edited, ...others] = bodies;
    assert(edited);
    try {
      edited.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
      edited.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 4, 0] });
      worker.cancelled.length = 0;
      const { shapes, swapped } = scopedSync([edited]);
      assert.deepEqual(shapes, [edited.lastInfo.shapeId], 'one tessellation, for the given shape');
      assert.deepEqual(worker.cancelled, [edited.lastInfo.shapeId], 'only the given shape loses its worker job');
      assert.deepEqual([...swapped].filter((body) => body !== edited), [], 'no other body swapped');
      assert.equal(current(edited), true, 'the given body has its current record');
      assert.equal(others.filter((body) => body.record !== undefined).length, 0, 'no other body changed');
      assert.equal(placed(edited, edited.surface), true, 'surface matrixWorld is current with no second walk');
      assert.equal(placed(edited, edited.outline), true, 'outline matrixWorld is current with no second walk');
    } finally {
      OpenGeometry.reset();
    }
  });
}

test('a scoped sync flush meshes and places a body that was never walked', async () => {
  await fresh();
  const { bodies } = await pendingScene(50);
  const added = box(2, 500);
  const scene = new THREE.Scene();
  scene.add(added);
  try {
    added.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 0, 3] });
    const { shapes } = scopedSync([added]);
    assert.deepEqual(shapes, [added.lastInfo.shapeId]);
    assert.equal(current(added), true);
    assert.equal(placed(added, added.surface), true);
    assert.equal(bodies.filter((body) => body.record !== undefined).length, 0);
  } finally {
    OpenGeometry.reset();
  }
});

test('a scoped sync flush gives the new record to displayed instances of the given shape', async () => {
  await fresh();
  const { bodies, scene } = await pendingScene(20);
  const [source] = bodies;
  assert(source);
  const copy = source.instance();
  scene.add(copy);
  scene.updateMatrixWorld(true);
  await nextTask();
  try {
    const { shapes } = scopedSync([source]);
    assert.deepEqual(shapes, [source.lastInfo.shapeId]);
    assert.equal(current(source), true);
    assert.equal(copy.record, source.record);
    assert.equal(bodies.slice(1).filter((body) => body.record !== undefined).length, 0);
  } finally {
    OpenGeometry.reset();
  }
});

test('a scoped sync flush skips hidden bodies and refuses disposed ones before meshing', async () => {
  await fresh();
  const { bodies } = await pendingScene(10);
  const [hidden, kept, gone] = bodies;
  assert(hidden && kept && gone);
  try {
    hidden.visible = false;
    assert.deepEqual(scopedSync([hidden]).shapes, []);
    assert.equal(hidden.record, undefined);
    gone.dispose();
    const log = tessellationLog();
    try {
      const both = Object({ geometry: 'sync', bodies: [kept, gone] });
      assert.throws(() => OpenGeometry.flush(both), failsWith('Disposed'));
    } finally {
      log.stop();
    }
    assert.deepEqual(log.shapes, [], 'nothing was meshed');
    assert.throws(() => OpenGeometry.flush(Object({ bodies: [kept] })), failsWith('InvalidParameter'));
    assert.equal(kept.record, undefined);
  } finally {
    OpenGeometry.reset();
  }
});

test('a sync flush without bodies still meshes every displayed body', async () => {
  await fresh();
  const { bodies } = await pendingScene(30);
  try {
    const { shapes } = scopedSync(undefined);
    assert.equal(shapes.length, bodies.length);
    assert(bodies.every(current));
  } finally {
    OpenGeometry.reset();
  }
});

test('a scoped sync flush refuses bodies that is not an array', async () => {
  await fresh();
  const { bodies } = await pendingScene(5);
  try {
    const single = Object({ geometry: 'sync', bodies: bodies[0] });
    assert.throws(() => OpenGeometry.flush(single), failsWith('InvalidParameter'));
    assert.throws(() => OpenGeometry.flush(Object({ geometry: 'sync', bodies: 'all' })), failsWith('InvalidParameter'));
    assert(bodies.every((body) => body.record === undefined));
  } finally {
    OpenGeometry.reset();
  }
});

test('a scoped sync flush refuses an item that is not a body', async () => {
  await fresh();
  const { bodies } = await pendingScene(5);
  const assembly = new SystemAssembly();
  try {
    for (const item of [null, 7, { handle: 0, generation: 0 }, new THREE.Group(), assembly]) {
      const options = Object({ geometry: 'sync', bodies: [bodies[0], item] });
      assert.throws(() => OpenGeometry.flush(options), failsWith('InvalidParameter'), String(item));
    }
    assert(bodies.every((body) => body.record === undefined));
  } finally {
    OpenGeometry.reset();
  }
});
