import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  Solid, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_CYLINDER, OG_PRIMITIVE_POLYHEDRON, OG_PRIMITIVE_SPHERE,
} from '../../../dist/index.js';
import { boot } from './support.mjs';

const CUBE_FACES = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];

function turned([x, y, z]) {
  const length = Math.hypot(1, 2, 3);
  const [u, v, w] = [1 / length, 2 / length, 3 / length];
  const [sin, cos] = [Math.sin(37 * Math.PI / 180), Math.cos(37 * Math.PI / 180)];
  const along = (u * x + v * y + w * z) * (1 - cos);
  return [
    u * along + x * cos + (v * z - w * y) * sin + 0.3,
    v * along + y * cos + (w * x - u * z) * sin - 0.2,
    w * along + z * cos + (u * y - v * x) * sin + 0.5,
  ];
}

function classified(body, point) {
  return body.classifyPoint(point);
}

function cube(x0) {
  const points = Array.from({ length: 8 }, (_, index) => turned([x0 + (index & 1), (index >> 1) & 1, index >> 2]));
  return new Solid(OG_PRIMITIVE_POLYHEDRON, { points, faces: CUBE_FACES });
}

test('classifyPoint gives Boundary on a cylinder seam, rim and side, a sphere pole and a cuboid corner', async () => {
  await boot();
  const leg = new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.1, height: 0.7 });
  const ball = new Solid(OG_PRIMITIVE_SPHERE, { radius: 1 });
  const box = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 1, depth: 3 });
  const probes = [
    [leg, [0.1, 0.35, 0]], [leg, [0.1, 0.7, 0]], [leg, [0.1 * Math.SQRT1_2, 0.35, 0.1 * Math.SQRT1_2]],
    [ball, [0, 1, 0]], [ball, [0, -1, 0]], [box, [1, 1, 1.5]], [box, [-1, 0, -1.5]],
  ];
  assert.deepEqual(probes.map(([body, point]) => classified(body, point)), probes.map(() => 'Boundary'));
  [leg, ball, box].forEach((body) => { body.dispose(); });
});

test('classifyPoint gives Boundary on a face two rotated cubes share, against both cubes', async () => {
  await boot();
  const [first, second] = [cube(0), cube(1)];
  const points = [];
  for (let i = 1; i < 10; i += 1) {
    for (let j = 1; j < 10; j += 1) points.push(turned([1, i / 10, j / 10]));
  }
  for (const body of [first, second]) {
    assert.deepEqual(points.map((point) => classified(body, point)), points.map(() => 'Boundary'));
    body.dispose();
  }
});
