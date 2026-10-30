import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';
import {
  OpenGeometry, Solid, Wire, OG_OPERATION_EXTRUDE, OG_PRIMITIVE_CONE, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_CYLINDER,
  OG_PRIMITIVE_POLYHEDRON, OG_PRIMITIVE_PROFILE, OG_PRIMITIVE_SPHERE, OG_PRIMITIVE_TORUS,
} from '../../../dist/index.js';
import { boot } from './support.mjs';

const EAVES = [[-0.45, -0.45], [12.45, -0.45], [12.45, 8.45], [-0.45, 8.45]];
const RIDGE = [[4, 4], [8, 4]];
const ROOF_FACES = [
  [4, 5, 1, 0], [5, 2, 1], [5, 4, 3, 2], [4, 0, 3], [7, 11, 10, 6], [8, 11, 7], [9, 10, 11, 8], [6, 10, 9],
  [0, 1, 7, 6], [1, 2, 8, 7], [2, 3, 9, 8], [3, 0, 6, 9],
];

function meshBox(body) {
  const scene = new THREE.Scene();
  scene.add(body);
  scene.updateMatrixWorld(true);
  OpenGeometry.flush({ geometry: 'sync' });
  const box = new THREE.Box3().setFromObject(body.surface, true);
  return [...box.min.toArray(), ...box.max.toArray()];
}

function assertBoundsHoldTheMesh(body) {
  try {
    const mesh = meshBox(body);
    const bounds = body.getBounds();
    assert(bounds);
    bounds.forEach((value, index) => {
      const near = mesh[index] ?? Number.NaN;
      const outward = index < 3 ? near - value : value - near;
      assert(outward >= -1e-6 && outward <= 0.01, `bounds ${bounds.join(', ')} against mesh ${mesh.join(', ')}`);
    });
  } finally {
    body.dispose();
  }
}

function polyhedron(points, faces) {
  return new Solid(OG_PRIMITIVE_POLYHEDRON, { points, faces });
}

function hipRoof() {
  const layer = (corners, y) => corners.map(([x, z]) => [x, y, z]);
  const points = [...layer(EAVES, 7.379), ...layer(RIDGE, 9.214), ...layer(EAVES, 7.179), ...layer(RIDGE, 9.014)];
  return polyhedron(points, ROOF_FACES);
}

function turnedBox(angle) {
  const [sin, cos] = [Math.sin(angle), Math.cos(angle)];
  const points = Array.from({ length: 8 }, (_, index) => {
    const [x, y, z] = [index & 1 ? 1 : -1, index & 2 ? 2 : 0, index & 4 ? 0.5 : -0.5];
    return [x * cos + z * sin, y, z * cos - x * sin];
  });
  return polyhedron(points, [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]]);
}

function quarterWall() {
  const arc = (to, counterClockwise) => ({ to, arc: { center: [0, 0], counterClockwise } });
  const profile = new Wire(OG_PRIMITIVE_PROFILE, {
    start: [2.15, 0],
    segments: [arc([0, 2.15], true), { to: [0, 1.85] }, arc([1.85, 0], false), { to: [2.15, 0] }],
    closed: true,
  });
  try {
    return new Solid(OG_OPERATION_EXTRUDE, { profile, distance: 3 });
  } finally {
    profile.dispose();
  }
}

test('a hip roof, a tetrahedron with a sloped face and a turned box are bounded by their mesh', async () => {
  await boot();
  const tetrahedron = polyhedron(
    [[0, 0, 0], [4, 0, 0], [0, 0, 4], [0, 3, 0]], [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
  );
  for (const body of [hipRoof(), tetrahedron, turnedBox(0.5)]) {
    assertBoundsHoldTheMesh(body);
  }
});

test('a quarter curved wall is bounded by its mesh, not by the box of its full circle', async () => {
  await boot();
  assertBoundsHoldTheMesh(quarterWall());
});

test('a cuboid, cylinder, sphere, cone and torus stay bounded by their mesh', async () => {
  await boot();
  const bodies = [
    new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 2, depth: 2 }),
    new Solid(OG_PRIMITIVE_CYLINDER, { radius: 0.3, height: 0.6 }),
    new Solid(OG_PRIMITIVE_SPHERE, { radius: 1 }),
    new Solid(OG_PRIMITIVE_CONE, { radiusBottom: 1, radiusTop: 0.5, height: 2 }),
    new Solid(OG_PRIMITIVE_TORUS, { majorRadius: 2, minorRadius: 0.5 }),
  ];
  for (const body of bodies) {
    assertBoundsHoldTheMesh(body);
  }
});
