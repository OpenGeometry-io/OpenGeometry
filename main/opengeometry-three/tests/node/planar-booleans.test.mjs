import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OGError, Solid, OG_OPERATION_INTERSECT, OG_OPERATION_SUBTRACT, OG_OPERATION_UNION, OG_PRIMITIVE_CUBOID,
  OG_PRIMITIVE_POLYHEDRON, OG_TRANSFORM_ROTATE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { boot } from './support.mjs';

const GABLE_FACES = [[0, 1, 2, 3], [1, 0, 4, 5], [2, 1, 5], [3, 2, 5, 4], [0, 3, 4]];
const CROSS_FACES = [[0, 1, 2, 3], [1, 0, 4], [3, 2, 5], [0, 3, 5, 4], [2, 1, 4, 5]];

function gableRoof(ogId) {
  const points = [[0, 0, 0], [4, 0, 0], [4, 0, 2], [0, 0, 2], [0, 1, 1], [4, 1, 1]];
  return new Solid(OG_PRIMITIVE_POLYHEDRON, { points, faces: GABLE_FACES }, { ogId });
}

function crossGable(ogId) {
  const points = [[1.5, -0.5, -1], [2.5, -0.5, -1], [2.5, -0.5, 3], [1.5, -0.5, 3], [2, 1.5, -1], [2, 1.5, 3]];
  return new Solid(OG_PRIMITIVE_POLYHEDRON, { points, faces: CROSS_FACES }, { ogId });
}

function turnedCube(ogId) {
  const cube = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId });
  cube.transform(OG_TRANSFORM_ROTATE, { axis: [0, 1, 0], degrees: 30, pivot: [0, 0, 0] });
  cube.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
  return cube;
}

function operated(host, operation, tool, expected) {
  try {
    host.operate(operation, { tools: [tool] });
    assert.deepEqual(host.validity(), { valid: true, closed: true, bodyType: 'Solid' });
    const { volume, errorBound } = host.measure();
    assert(
      Math.abs(volume - expected) <= errorBound,
      `${host.ogId}: ${volume} versus ${expected} with bound ${errorBound}`,
    );
    return host.getReport()?.handlers;
  } finally {
    tool.dispose();
    host.dispose();
  }
}

test('a gable roof and a crossing gable sunk below its eaves union into one closed solid', async () => {
  await boot();
  const handlers = operated(gableRoof('union-roof'), OG_OPERATION_UNION, crossGable('union-cross'), 89 / 12);
  assert.deepEqual(handlers, ['boolean_planar_polyhedra']);
});

test('a gable roof and a crossing gable sunk below its eaves intersect into one closed solid', async () => {
  await boot();
  const handlers = operated(gableRoof('meet-roof'), OG_OPERATION_INTERSECT, crossGable('meet-cross'), 7 / 12);
  assert.deepEqual(handlers, ['boolean_planar_polyhedra']);
});

test('a gable roof minus a crossing gable still goes through the planar polyhedra subtraction', async () => {
  await boot();
  const handlers = operated(gableRoof('cut-roof'), OG_OPERATION_SUBTRACT, crossGable('cut-cross'), 41 / 12);
  assert.deepEqual(handlers, ['subtract_planar_polyhedra']);
});

test('a cuboid and a cube turned 30 degrees about its height union and intersect', async () => {
  await boot();
  const slab = (ogId) => new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 2 }, { ogId });
  const union = operated(slab('turned-union-slab'), OG_OPERATION_UNION, turnedCube('turned-union-cube'), 4.5);
  assert.deepEqual(union, ['boolean_planar_polyhedra']);
  const meet = operated(slab('turned-meet-slab'), OG_OPERATION_INTERSECT, turnedCube('turned-meet-cube'), 0.5);
  assert.deepEqual(meet, ['boolean_planar_polyhedra']);
});

test('a block standing on the floor wholly inside a gable roof subtracts to nothing', async () => {
  await boot();
  const block = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.4, height: 0.3, depth: 0.4 }, { ogId: 'inside-block' });
  block.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 1] });
  const roof = gableRoof('inside-roof');
  try {
    assert.throws(
      () => block.operate(OG_OPERATION_SUBTRACT, { tools: [roof] }),
      (error) => error instanceof OGError && error.code === 'EmptyResult',
    );
    assert(Math.abs(block.measure().volume - 0.048) <= block.measure().errorBound);
  } finally {
    roof.dispose();
    block.dispose();
  }
});
