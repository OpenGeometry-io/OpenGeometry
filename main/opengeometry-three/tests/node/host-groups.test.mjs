import assert from 'node:assert/strict';
import { test } from 'node:test';
import { setFlagsFromString } from 'node:v8';
import { runInNewContext } from 'node:vm';
import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID } from '../../../dist/index.js';
import { displayed, runtime } from '../../../dist/testing.js';
import { boot, cuboid, failsWith, nextTask } from './support.mjs';

const RENDERER = { info: { render: { frame: 0 } }, getDrawingBufferSize: (target) => target.set(1000, 1000) };
const CAMERA = new THREE.PerspectiveCamera(50, 1, 0.1, 1000);
const BODY = Object.getPrototypeOf(Solid.prototype);
setFlagsFromString('--expose-gc');
const COLLECT = runInNewContext('gc');

async function fresh() {
  await boot();
  OpenGeometry.reset();
  CAMERA.position.set(0, 0, 10);
  CAMERA.updateMatrixWorld();
}

function ortho(target) {
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000);
  camera.position.set(0, 0, 10);
  camera.zoom = 1 / (1000 * target);
  camera.updateProjectionMatrix();
  camera.updateMatrixWorld();
  return camera;
}

function observe(camera) {
  RENDERER.info.render.frame++;
  Reflect.apply(OpenGeometry.observeCamera, OpenGeometry, [RENDERER, camera]);
}

function current(body) {
  return body.record?.shapeId === body.lastInfo.shapeId && body.record?.revision === body.lastInfo.shapeRevision;
}

async function drawFrame(groups) {
  for (const group of groups) group.drawn();
  await nextTask();
}

function counted(target, name) {
  const original = target[name];
  const log = { calls: 0, stop: () => { Reflect.deleteProperty(target, name); } };
  target[name] = (...args) => { log.calls++; return Reflect.apply(original, target, args); };
  return log;
}

function installsOn(body, step) {
  const install = BODY.install;
  let calls = 0;
  BODY.install = function countedInstall(...rest) {
    if (body === undefined || this === body) calls++;
    return Reflect.apply(install, this, rest);
  };
  try {
    step();
  } finally {
    BODY.install = install;
  }
  return calls;
}

function asyncProvider() {
  const provider = Object(runtime().provider);
  const compute = provider.compute.bind(provider);
  provider.compute = undefined;
  provider.request = (request) => new Promise((resolve) => { setTimeout(() => { resolve(compute(request)); }, 1); });
  return () => { delete provider.compute; delete provider.request; };
}

function memberIds() {
  return [...runtime().groups.live].flatMap((group) => [...group.members].map((body) => body.ogId));
}

async function steadyPassWork(size) {
  await fresh();
  const bodies = Array.from({ length: size }, (_, index) => cuboid(`work-${String(size)}-${String(index)}`));
  const group = OpenGeometry.hostGroup(bodies);
  try {
    for (let index = 0; index < 2; index++) {
      observe(CAMERA);
      await drawFrame([group]);
    }
    assert.equal(bodies.every((body) => displayed(body) && current(body)), true, 'every member is drawn');
    const state = runtime();
    state.lod.lastSchedule = 0;
    observe(CAMERA);
    const logs = [counted(state.groups.of, 'get'), counted(state.records, 'get'), counted(state.bodies, 'get')];
    let installs = 0;
    try {
      installs = installsOn(undefined, () => { group.drawn(); });
      await Promise.resolve();
    } finally {
      for (const log of logs) log.stop();
    }
    const evaluated = state.lod.lastSchedule > 0;
    return { lookups: logs.map((log) => log.calls), installs, active: state.renderPassActive, evaluated };
  } finally {
    OpenGeometry.reset();
  }
}

test('drawn() displays every member of a 1,000-body group and a later pass does no work per member', async () => {
  const small = await steadyPassWork(10);
  const large = await steadyPassWork(1000);
  assert.deepEqual(large, small);
  assert.deepEqual(large.lookups, [0, 0, 0]);
  assert.equal(large.installs, 0);
  assert.equal(large.active, false, 'the pass ends');
  assert.equal(large.evaluated, true, 'the level-of-detail evaluation runs in the measured pass');
});

test('a group not drawn for two passes leaves its members undisplayed and they get no swap', async () => {
  await fresh();
  const [gone, kept] = [cuboid('group-gone'), cuboid('group-kept', 2)];
  const [goneGroup, keptGroup] = [OpenGeometry.hostGroup([gone]), OpenGeometry.hostGroup([kept])];
  try {
    await drawFrame([goneGroup, keptGroup]);
    assert.equal(displayed(gone), true);
    assert.equal(current(gone), true);
    await drawFrame([keptGroup]);
    await drawFrame([keptGroup]);
    assert.equal(displayed(gone), false);
    const installs = installsOn(gone, () => {
      gone.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
      OpenGeometry.flush();
    });
    await drawFrame([keptGroup]);
    assert.equal(installs, 0);
    assert.equal(current(gone), false);
  } finally {
    OpenGeometry.reset();
  }
});

test('worker geometry is installed on a member of a drawn group and not on one of an undrawn group', async () => {
  await fresh();
  const restore = asyncProvider();
  const shown = cuboid('arrive-shown');
  const idle = shown.instance({ ogId: 'arrive-idle' });
  OpenGeometry.flush();
  const shownGroup = OpenGeometry.hostGroup([shown]);
  OpenGeometry.hostGroup([idle]);
  try {
    assert.equal(idle.lastInfo.shapeId, shown.lastInfo.shapeId);
    shownGroup.drawn();
    assert.equal(shown.record, undefined, 'no install inside the draw');
    for (let index = 0; index < 10 && !current(shown); index++) await drawFrame([shownGroup]);
    assert.equal(current(shown), true);
    assert.equal(displayed(idle), false);
    assert.equal(idle.record, undefined);
  } finally {
    restore();
    OpenGeometry.reset();
  }
});

test('a member of a drawn group gets new geometry after its shape changes', async () => {
  await fresh();
  const body = cuboid('group-edit');
  const group = OpenGeometry.hostGroup([body]);
  try {
    await drawFrame([group]);
    const before = body.record;
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 4, height: 1, depth: 1 });
    await drawFrame([group]);
    assert.notEqual(body.record, before);
    assert.equal(current(body), true);
  } finally {
    OpenGeometry.reset();
  }
});

test('settled, a sync flush and level of detail see a member of a drawn group', async () => {
  await fresh();
  const body = cuboid('group-settle');
  const group = OpenGeometry.hostGroup([body]);
  try {
    Reflect.apply(OpenGeometry.observeCamera, OpenGeometry, [RENDERER, CAMERA]);
    await drawFrame([group]);
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 1 });
    OpenGeometry.flush({ geometry: 'sync' });
    assert.equal(current(body), true, 'a sync flush installs a group member');
    const { failed } = await OpenGeometry.settled();
    assert.deepEqual(failed, []);
    assert.equal(typeof runtime().lod.shapes.get(String(body.lastInfo.shapeId))?.target, 'number');
  } finally {
    OpenGeometry.reset();
  }
});

test('set() replaces the members and dispose() stops the group counting', async () => {
  await fresh();
  const [first, second] = [cuboid('set-first'), cuboid('set-second', 2)];
  const group = OpenGeometry.hostGroup([first]);
  try {
    await drawFrame([group]);
    assert.equal(displayed(first), true);
    assert.equal(displayed(second), false);
    group.set([second, second]);
    assert.equal(displayed(first), false);
    assert.equal(displayed(second), true);
    assert.deepEqual(memberIds(), ['set-second']);
    await drawFrame([group]);
    assert.equal(current(second), true);
    group.dispose();
    assert.equal(displayed(second), false);
    group.drawn();
    assert.equal(runtime().renderPassActive, false, 'a disposed group starts no pass');
    assert.throws(() => group.set([first]), failsWith('Disposed'));
    group.dispose();
    assert.deepEqual(memberIds(), []);
  } finally {
    OpenGeometry.reset();
  }
});

test('a body in two groups is displayed while either group is drawn', async () => {
  await fresh();
  const body = cuboid('two-groups');
  const [one, two] = [OpenGeometry.hostGroup([body]), OpenGeometry.hostGroup([body])];
  const other = OpenGeometry.hostGroup([cuboid('two-other', 2)]);
  try {
    await drawFrame([two]);
    assert.equal(displayed(body), true);
    two.dispose();
    assert.equal(displayed(body), false);
    await drawFrame([one]);
    assert.equal(displayed(body), true);
    await drawFrame([other]);
    await drawFrame([other]);
    assert.equal(displayed(body), false);
  } finally {
    OpenGeometry.reset();
  }
});

test('drawn() skips a disposed member and the group drops it', async () => {
  await fresh();
  const [kept, gone] = [cuboid('member-kept'), cuboid('member-gone', 2)];
  const group = OpenGeometry.hostGroup([kept, gone]);
  try {
    gone.dispose();
    await drawFrame([group]);
    assert.equal(current(kept), true);
    assert.equal(displayed(gone), false);
    assert.deepEqual(memberIds(), ['member-kept']);
  } finally {
    OpenGeometry.reset();
  }
});

test('hostGroup and set reject what markDisplayed rejects, and a reset ends the group', async () => {
  await fresh();
  const body = cuboid('group-input');
  const disposed = cuboid('group-disposed', 2);
  disposed.dispose();
  const invalid = failsWith('InvalidParameter');
  assert.throws(() => Reflect.apply(OpenGeometry.hostGroup, OpenGeometry, [body]), invalid);
  assert.throws(() => Reflect.apply(OpenGeometry.hostGroup, OpenGeometry, [[new THREE.Mesh()]]), invalid);
  assert.throws(() => OpenGeometry.hostGroup([disposed]), failsWith('Disposed'));
  const group = OpenGeometry.hostGroup([]);
  assert.throws(() => Reflect.apply(group.set, group, [[body, new THREE.Mesh()]]), invalid);
  assert.throws(() => group.set([disposed]), failsWith('Disposed'));
  group.set([body]);
  OpenGeometry.reset();
  group.drawn();
  assert.equal(runtime().renderPassActive, false);
  assert.throws(() => group.set([]), failsWith('Disposed'));
  OpenGeometry.reset();
});

test('a host-drawn member of a drawn group keeps its own visibility', async () => {
  await fresh();
  const [hosted, hidden] = [cuboid('group-hosted'), cuboid('group-hidden', 2)];
  hidden.visible = false;
  OpenGeometry.setHostDrawn([hosted, hidden], true);
  const group = OpenGeometry.hostGroup([hosted, hidden]);
  try {
    await drawFrame([group]);
    assert.equal(displayed(hosted), true);
    assert.equal(current(hosted), true);
    assert.equal(displayed(hidden), false);
    assert.equal(hidden.record, undefined);
  } finally {
    OpenGeometry.reset();
  }
});

test('a member of a drawn group returns to its own bucket after settled with a deflection', async () => {
  await fresh();
  const camera = ortho(2 ** -10);
  const [stamped, member] = [cuboid('settle-stamped'), cuboid('settle-member')];
  const group = OpenGeometry.hostGroup([member]);
  const frames = async () => {
    for (let index = 0; index < 4; index++) {
      runtime().lod.lastSchedule = 0;
      observe(camera);
      OpenGeometry.markDisplayed([stamped]);
      await drawFrame([group]);
    }
  };
  try {
    await frames();
    const resting = stamped.record?.bucket;
    assert.equal(member.record?.bucket, resting);
    await OpenGeometry.settled({ deflection: 0.05 });
    assert.notEqual(member.record?.bucket, resting, 'settled meshes at its deflection');
    await frames();
    assert.equal(stamped.record?.bucket, resting);
    assert.equal(member.record?.bucket, resting);
  } finally {
    OpenGeometry.reset();
  }
});

test('a host-drawn member shown again while its group is drawn gets its geometry', async () => {
  await fresh();
  const member = cuboid('shown-later');
  member.visible = false;
  OpenGeometry.setHostDrawn([member], true);
  const group = OpenGeometry.hostGroup([member]);
  try {
    await drawFrame([group]);
    await drawFrame([group]);
    assert.equal(member.record, undefined);
    OpenGeometry.setHostDrawn([member], false);
    member.visible = true;
    OpenGeometry.setHostDrawn([member], true);
    await drawFrame([group]);
    assert.equal(displayed(member), true);
    assert.equal(current(member), true);
  } finally {
    OpenGeometry.reset();
  }
});

test('a disposed member leaves an undrawn group at once', async () => {
  await fresh();
  const [kept, gone] = [cuboid('undrawn-kept'), cuboid('undrawn-gone', 2)];
  OpenGeometry.hostGroup([kept, gone]);
  try {
    gone.dispose();
    assert.deepEqual(memberIds(), ['undrawn-kept']);
  } finally {
    OpenGeometry.reset();
  }
});

test('a host group dropped without dispose is released by the garbage collector', async () => {
  await fresh();
  const body = cuboid('dropped-group');
  (() => { OpenGeometry.hostGroup([body]).drawn(); })();
  try {
    await nextTask();
    for (let index = 0; index < 20 && runtime().groups.live.size > 0; index++) {
      Reflect.apply(Object(COLLECT), undefined, []);
      await nextTask();
    }
    assert.equal(runtime().groups.live.size, 0);
    assert.equal(displayed(body), false);
  } finally {
    OpenGeometry.reset();
  }
});
