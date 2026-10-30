import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, SystemAssembly, OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { ensureGeometry, graph, runtime } from '../../../dist/testing.js';
import { boot, cuboid, nextTask } from './support.mjs';

const LIVE = 300;
const RENDERER = { getDrawingBufferSize: (target) => target.set(1000, 1000) };
const BODY = Object.getPrototypeOf(Solid.prototype);

async function fresh() {
  await boot();
  OpenGeometry.reset();
}

function box(ogId, width = 1) {
  return new Solid(OG_PRIMITIVE_CUBOID, { width, height: 1, depth: 1 }, ogId ? { ogId } : {});
}

function swapLog() {
  const swap = BODY.swapReadyRecord;
  const log = { calls: 0, stop: () => { BODY.swapReadyRecord = swap; } };
  BODY.swapReadyRecord = function counted(...rest) {
    log.calls++;
    return Reflect.apply(swap, this, rest);
  };
  return log;
}

function bucketLog() {
  const target = Object(graph());
  const read = target.displayBuckets;
  const log = { calls: 0, stop: () => { delete target.displayBuckets; } };
  target.displayBuckets = (...rest) => { log.calls++; return Reflect.apply(read, target, rest); };
  return log;
}

function swapsDuring(step) {
  const log = swapLog();
  try {
    step();
  } finally {
    log.stop();
  }
  return log.calls;
}

async function shownScene(count, prefix) {
  const bodies = Array.from({ length: count }, (_, index) => cuboid(`${prefix}-${String(index)}`, 1 + index / 1000));
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  await nextTask();
  return { scene, bodies };
}

function current(body) {
  return body.record?.shapeId === body.lastInfo.shapeId && body.record?.revision === body.lastInfo.shapeRevision;
}

test('an edit swaps only the bodies in its change set', async () => {
  await fresh();
  const { bodies } = await shownScene(LIVE, 'edit');
  const [first, second, third] = bodies;
  const extra = [box(), box(undefined, 2)];
  const edits = {
    rebuild: () => first?.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 }),
    mirror: () => second?.mirror({ origin: [0, 0, 0], normal: [1, 0, 0] }),
    makeUnique: () => third?.instance().makeUnique(),
    'assembly dispose': () => {
      const assembly = new SystemAssembly();
      assembly.addChild(extra);
      assembly.dispose();
    },
    rollback: () => {
      const mark = OpenGeometry.mark();
      first?.rebuild(OG_PRIMITIVE_CUBOID, { width: 4, height: 1, depth: 1 });
      mark.rollback();
      mark.release();
    },
  };
  try {
    const counts = Object.fromEntries(Object.entries(edits).map(([name, edit]) => [name, swapsDuring(edit)]));
    const over = Object.entries(counts).filter(([, calls]) => calls > 2);
    assert.deepEqual(over, [], `swaps per edit with ${String(LIVE)} displayed bodies: ${JSON.stringify(counts)}`);
  } finally {
    OpenGeometry.reset();
  }
});

test('a record received outside a render pass swaps only the bodies of its shape', async () => {
  await fresh();
  const { bodies } = await shownScene(LIVE, 'arrive');
  const [edited] = bodies;
  assert(edited);
  try {
    edited.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    await nextTask();
    assert.equal(current(edited), false);
    const swaps = swapsDuring(() => ensureGeometry(edited));
    assert.equal(current(edited), true, 'the displayed body got its record');
    assert(swaps <= 1, `${String(swaps)} swaps for one arrival with ${String(LIVE)} displayed bodies`);
  } finally {
    OpenGeometry.reset();
  }
});

test('a render pass after an edit swaps only the edited bodies', async () => {
  await fresh();
  const { scene, bodies } = await shownScene(LIVE, 'pass');
  const [moved, edited] = bodies;
  try {
    moved?.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    edited?.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    const log = swapLog();
    try {
      scene.updateMatrixWorld(true);
      await nextTask();
    } finally {
      log.stop();
    }
    assert.equal(current(edited), true);
    assert(log.calls <= 3, `${String(log.calls)} swaps in one pass with ${String(LIVE)} displayed bodies`);
  } finally {
    OpenGeometry.reset();
  }
});

test('settled after an edit swaps only the edited body', async () => {
  await fresh();
  const { bodies } = await shownScene(LIVE, 'settle');
  const [edited] = bodies;
  try {
    edited?.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    const log = swapLog();
    let failed;
    try {
      ({ failed } = await OpenGeometry.settled());
    } finally {
      log.stop();
    }
    assert.deepEqual(failed, []);
    assert.equal(current(edited), true);
    assert(log.calls <= 2, `${String(log.calls)} swaps in settled with ${String(LIVE)} displayed bodies`);
  } finally {
    OpenGeometry.reset();
  }
});

test('create, read matrixWorld, dispose stays flat in swaps (raw repro step)', async () => {
  await fresh();
  for (let index = 0; index < LIVE; index++) box(undefined, 1 + index / 1000);
  const steps = 10;
  try {
    const swaps = swapsDuring(() => {
      for (let step = 0; step < steps; step++) {
        const body = box();
        body.updateMatrixWorld(true);
        body.dispose();
      }
    });
    assert(swaps <= 2 * steps, `${String(swaps / steps)} swaps per step with ${String(LIVE)} live bodies`);
  } finally {
    await nextTask();
    OpenGeometry.reset();
  }
});

test('bodies that are not displayed never read their display buckets', async () => {
  await fresh();
  const live = Array.from({ length: LIVE }, (_, index) => box(undefined, 1 + index / 1000));
  const [edited, host, tool] = live;
  assert(edited && host && tool);
  const log = bucketLog();
  try {
    edited.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [0.5, 0, 0] });
    host.operate(OG_OPERATION_SUBTRACT, { tools: [tool] });
    OpenGeometry.flush();
    box().dispose();
  } finally {
    log.stop();
    OpenGeometry.reset();
  }
  assert.equal(log.calls, 0, `${String(log.calls)} displayBuckets calls, no body displayed`);
});

test('a display clone released after reset cannot evict a live record of the new pool', async () => {
  await fresh();
  const before = await shownScene(5, 'pool');
  const clone = before.bodies[0]?.clone();
  const stale = clone ? Reflect.get(clone, 'sourceRecord') : undefined;
  assert(clone && stale);
  OpenGeometry.reset();
  await shownScene(5, 'pool');
  const pool = Object(runtime().records);
  const live = pool.records.get(stale.key);
  try {
    assert(live && live !== stale && live.refs > 0, 'the new pool holds a live record under the same key');
    clone.dispose();
    const foreign = [...pool.idle].filter((record) => pool.records.get(record.key) !== record);
    assert.deepEqual(foreign.map((record) => record.key), [], 'idle holds only records of this pool');
    pool.idleBytes = Number.POSITIVE_INFINITY;
    pool.evict(live.shapeId);
    assert.equal(pool.records.get(live.key), live, 'the live record survives eviction');
    assert.equal(pool.byShape.get(live.shapeId)?.has(live), true);
  } finally {
    OpenGeometry.reset();
  }
});

const ROUNDS = 5;
const EDITS = 40;

function camera() {
  const view = new THREE.PerspectiveCamera(50, 1, 0.1, 1e6);
  view.position.set(0, 0, 200);
  view.lookAt(0, 0, 0);
  view.updateMatrixWorld();
  return view;
}

async function frame(scene, view) {
  scene.updateMatrixWorld(true);
  const mesh = scene.children[0]?.surface;
  if (mesh) Reflect.apply(mesh.onBeforeRender, mesh, [RENDERER, scene, view]);
  await nextTask();
}

async function resize(world, target) {
  while (world.live.length > target) world.live.pop()?.dispose();
  while (world.live.length < target) {
    const body = box(undefined, 1 + (world.live.length % 997) / 1000);
    world.live.push(body);
    world.scene.add(body);
  }
  await frame(world.scene, world.view);
  assert.deepEqual((await OpenGeometry.settled()).failed, []);
}

function least(values) {
  return Math.min(...values);
}

async function rebuildCost(world, round) {
  const edited = world.live.slice(0, EDITS);
  const start = performance.now();
  for (const body of edited) body.rebuild(OG_PRIMITIVE_CUBOID, { width: 2 + round / 10, height: 1, depth: 1 });
  return (performance.now() - start) / EDITS;
}

async function settledCost(world, round) {
  for (const body of world.live.slice(EDITS, 2 * EDITS)) {
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 2 + round / 10, height: 1, depth: 1 });
  }
  const start = performance.now();
  await OpenGeometry.settled();
  return (performance.now() - start) / world.live.length;
}

async function disposeCost(world) {
  const extra = Array.from({ length: EDITS }, (_, index) => box(undefined, 3 + index / 100));
  world.scene.add(...extra);
  await frame(world.scene, world.view);
  await OpenGeometry.settled();
  assert(extra.every((body) => body.record !== undefined));
  const start = performance.now();
  for (const body of extra) body.dispose();
  return (performance.now() - start) / EDITS;
}

async function costs(world) {
  /** @type {{ rebuild: number[], settled: number[], dispose: number[] }} */
  const sample = { rebuild: [], settled: [], dispose: [] };
  for (let round = 0; round < ROUNDS; round++) {
    sample.rebuild.push(await rebuildCost(world, round));
    sample.settled.push(await settledCost(world, round));
    sample.dispose.push(await disposeCost(world));
    await frame(world.scene, world.view);
    await OpenGeometry.settled();
  }
  return { rebuild: least(sample.rebuild), settled: least(sample.settled), dispose: least(sample.dispose) };
}

test('cost per body at 4,000 live bodies is at most 2x the cost at 1,000', async (context) => {
  await fresh();
  const world = { scene: new THREE.Scene(), view: camera(), live: [] };
  /** @type {Map<number, Map<string, number>>} */
  const best = new Map([[1000, new Map()], [4000, new Map()]]);
  try {
    for (const size of [1000, 4000, 1000, 4000]) {
      await resize(world, size);
      const measured = await costs(world);
      const kept = best.get(size) ?? new Map();
      for (const [name, value] of Object.entries(measured)) kept.set(name, least([kept.get(name) ?? Infinity, value]));
    }
  } finally {
    OpenGeometry.reset();
  }
  const small = best.get(1000) ?? new Map();
  const large = best.get(4000) ?? new Map();
  const rows = [...small].map(([name, low]) => ({ name, low, high: large.get(name) ?? Infinity }));
  const over = rows.filter(({ low, high }) => high > 2 * low).map(({ name }) => name);
  const detail = rows.map(({ name, low, high }) =>
    `${name}: ${low.toFixed(4)} -> ${high.toFixed(4)} ms (${(high / low).toFixed(2)}x)`);
  context.diagnostic(detail.join('; '));
  assert.deepEqual(over, [], detail.join('; '));
});
