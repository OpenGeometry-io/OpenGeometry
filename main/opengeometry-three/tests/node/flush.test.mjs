import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, OG_OPERATION_SHELL, OG_OPERATION_SUBTRACT, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { flushCount, graph, runtime } from '../../../dist/testing.js';
import { boot, cuboid, nextTask } from './support.mjs';

function cuboids(prefix) {
  return Array.from({ length: 50 }, (_, index) => cuboid(`${prefix}-${String(index)}`, 1 + index / 100));
}

function disposeAll(bodies) {
  for (const body of bodies) body.dispose();
}

test('flush after a transform makes no node call', async () => {
  await boot();
  await nextTask();
  const bodies = cuboids('node-call');
  OpenGeometry.flush();
  const target = Object(graph());
  const node = target.node;
  let calls = 0;
  target.node = (...args) => { calls++; return Reflect.apply(node, target, args); };
  try {
    bodies[0]?.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    OpenGeometry.flush();
  } finally {
    delete target.node;
    disposeAll(bodies);
  }
  assert.equal(calls, 0);
});

test('flush purges records only for the shapes in the change set', async () => {
  await boot();
  await nextTask();
  const bodies = cuboids('purge');
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  await nextTask();
  const records = Object(runtime().records);
  const purge = records.purge;
  let calls = 0;
  records.purge = (...args) => { calls++; return Reflect.apply(purge, records, args); };
  try {
    bodies[0]?.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
    OpenGeometry.flush();
  } finally {
    delete records.purge;
    disposeAll(bodies);
  }
  assert(calls <= 1, `${String(calls)} purge calls`);
});

async function shownScene(prefix) {
  const bodies = cuboids(prefix);
  const scene = new THREE.Scene();
  scene.add(...bodies);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  await nextTask();
  return { scene, bodies };
}

async function shown(prefix) {
  return (await shownScene(prefix)).bodies;
}

function revisionReadsDuring(step) {
  const target = Object(graph());
  const revision = target.revision;
  let calls = 0;
  target.revision = (...args) => { calls++; return Reflect.apply(revision, target, args); };
  try {
    step();
  } finally {
    delete target.revision;
  }
  return calls;
}

function swapsDuring(step) {
  const prototype = Object.getPrototypeOf(Solid.prototype);
  const swap = prototype.swapReadyRecord;
  let calls = 0;
  prototype.swapReadyRecord = function counted(...rest) { calls++; return Reflect.apply(swap, this, rest); };
  try {
    step();
  } finally {
    prototype.swapReadyRecord = swap;
  }
  return calls;
}

test('operate flushes with scope changed: only the operated bodies swap', async () => {
  await boot();
  await nextTask();
  const bodies = await shown('operate');
  const [host, tool, shelled] = bodies;
  assert(host && tool && shelled);
  try {
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [0.5, 0, 0] });
    const cut = swapsDuring(() => host.operate(OG_OPERATION_SUBTRACT, { tools: [tool] }));
    const shell = swapsDuring(() => shelled.operate(OG_OPERATION_SHELL, { thickness: 0.1 }));
    assert(cut <= 2, `${String(cut)} swaps for one subtract with ${String(bodies.length)} displayed bodies`);
    assert(shell <= 1, `${String(shell)} swaps for one shell with ${String(bodies.length)} displayed bodies`);
  } finally {
    disposeAll(bodies);
  }
});

test('one render pass over 50 displayed bodies reads the graph revision once', async () => {
  await boot();
  await nextTask();
  const { scene, bodies } = await shownScene('one-read');
  try {
    const reads = revisionReadsDuring(() => scene.updateMatrixWorld(true));
    assert.equal(reads, 1, `${String(reads)} revision reads in one pass over ${String(bodies.length)} bodies`);
    await nextTask();
  } finally {
    disposeAll(bodies);
  }
});

test('a graph change between two passes is applied before the next pass installs geometry', async () => {
  await boot();
  await nextTask();
  const { scene, bodies } = await shownScene('between');
  const [edited] = bodies;
  assert(edited);
  const late = cuboid('between-late', 2);
  try {
    edited.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    edited.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 0] });
    scene.add(late);
    const flushes = flushCount();
    const installs = [];
    edited.install = (record) => {
      installs.push({ revision: record.revision, current: edited.lastInfo.shapeRevision, flushes: flushCount() });
      return Reflect.apply(Object.getPrototypeOf(edited).install, edited, [record]);
    };
    scene.updateMatrixWorld(true);
    Reflect.deleteProperty(edited, 'install');
    assert.equal(flushCount(), flushes + 1);
    assert.equal(installs[0]?.flushes, flushes + 1);
    assert.equal(installs[0]?.revision, installs[0]?.current);
    assert.equal(edited.matrixWorld.elements[12], 2);
    assert.equal(late.record?.revision, late.lastInfo.shapeRevision);
    await nextTask();
  } finally {
    disposeAll([...bodies, late]);
  }
});

test('a transform after a pass started in the same task moves the body at its next matrix update', async () => {
  await boot();
  await nextTask();
  const { scene, bodies } = await shownScene('mid-transform');
  const [moved] = bodies;
  assert(moved);
  try {
    scene.updateMatrixWorld(true);
    moved.transform(OG_TRANSFORM_TRANSLATE, { offset: [5, 0, 0] });
    moved.updateMatrixWorld(true);
    assert.equal(moved.matrixWorld.elements[12], 5);
    await nextTask();
  } finally {
    disposeAll(bodies);
  }
});

test('an addChild after a pass started in the same task places the child at its next matrix update', async () => {
  await boot();
  await nextTask();
  const { scene, bodies } = await shownScene('mid-add-child');
  const [parent, child] = bodies;
  assert(parent && child);
  try {
    parent.transform(OG_TRANSFORM_TRANSLATE, { offset: [3, 0, 0] });
    OpenGeometry.flush();
    scene.updateMatrixWorld(true);
    parent.addChild([child]);
    child.updateMatrixWorld(true);
    assert.equal(child.matrixWorld.elements[12], 3);
    await nextTask();
  } finally {
    disposeAll(bodies.filter((body) => body !== child));
  }
});

test('a pass with one transform in its middle reads the graph revision twice', async () => {
  await boot();
  await nextTask();
  const { scene, bodies } = await shownScene('two-reads');
  const [first] = bodies;
  assert(first);
  try {
    const reads = revisionReadsDuring(() => {
      first.updateMatrixWorld(true);
      first.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
      scene.updateMatrixWorld(true);
    });
    assert.equal(reads, 2, `${String(reads)} revision reads in one pass with one transform`);
    await nextTask();
  } finally {
    disposeAll(bodies);
  }
});
