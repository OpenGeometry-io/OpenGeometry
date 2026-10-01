import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID } from '../../../../../../dist/index.js';
import * as TESTING from '../../../../../../dist/testing.js';
import { required } from '../../../support/test-page.js';
import { tessellateSends, type AcceptancePage } from '../../acceptance-probes.js';

type RetryWarning = { code: string; bucket: number; retryBucket: number };

function wallBucket(page: AcceptancePage): number {
  return required(page.wall.record, 'a wall record').bucket;
}

async function renderSettled(page: AcceptancePage): Promise<number> {
  page.renderer.render(page.scene, page.camera);
  await OpenGeometry.settled();
  return wallBucket(page);
}

function aimAt(camera: THREE.PerspectiveCamera, centre: THREE.Vector3): void {
  camera.lookAt(centre);
  camera.updateMatrixWorld();
}

export async function lodProbe(page: AcceptancePage): Promise<{ before: number; after: number }> {
  const before = await renderSettled(page);
  const centre = new THREE.Vector3(2, 3, 1);
  page.camera.position.sub(centre).multiplyScalar(0.25).add(centre);
  aimAt(page.camera, centre);
  return { before, after: await renderSettled(page) };
}

function placeForTarget(page: AcceptancePage, target: number): void {
  const { renderer, camera, wall } = page;
  const box = new THREE.Box3().setFromArray(required(wall.getBounds(), 'wall bounds'));
  const centre = box.getCenter(new THREE.Vector3());
  const extent = box.getSize(new THREE.Vector3()).length() / 2;
  const height = renderer.getDrawingBufferSize(new THREE.Vector2()).y;
  const depth = target * height / Math.tan(THREE.MathUtils.degToRad(camera.getEffectiveFOV()) / 2);
  const direction = camera.position.clone().sub(centre).normalize();
  camera.position.copy(centre).addScaledVector(direction, depth + extent);
  aimAt(camera, centre);
}

export async function lodHysteresisProbe(page: AcceptancePage): Promise<{ p: number; buckets: number[] }> {
  const home = page.camera.position.clone();
  const p = await renderSettled(page);
  const buckets: number[] = [];
  for (const target of [3.96 * p, 4.04 * p, 0.76 * 4 * p, 0.74 * 4 * p]) {
    placeForTarget(page, target);
    buckets.push(await renderSettled(page));
  }
  page.camera.position.copy(home);
  aimAt(page.camera, new THREE.Vector3(2, 3, 1));
  await renderSettled(page);
  return { p, buckets };
}

export async function orbitProbe(
  page: AcceptancePage,
): Promise<{ jobs: number; buckets: number[]; sendBuckets: number[] }> {
  const { renderer, scene, camera } = page;
  const shapeId = required(page.wall.lastInfo.shapeId, 'a wall shape');
  const buckets = [await renderSettled(page)];
  const before = tessellateSends(shapeId);
  const start = performance.now();
  OpenGeometry.setCameraMotion(true);
  while (performance.now() - start <= 150) {
    await new Promise<void>((resolve) => requestAnimationFrame(() => { resolve(); }));
    camera.position.x += 0.02;
    aimAt(camera, new THREE.Vector3(2, 3, 1));
    renderer.render(scene, camera);
    buckets.push(wallBucket(page));
  }
  await OpenGeometry.settled();
  OpenGeometry.setCameraMotion(false);
  buckets.push(wallBucket(page));
  const sends = TESTING.workerSendLog().filter((send) => send.shapeId === shapeId).slice(before);
  return { jobs: sends.length, buckets, sendBuckets: sends.map((send) => send.bucket) };
}

function pinWantedBucket(shapeId: string, bucket: number): void {
  const pin: unknown = Reflect.get(TESTING, 'pinWantedBucket');
  if (typeof pin !== 'function') throw new Error('Test page expected pinWantedBucket in dist/testing.js');
  Reflect.apply(pin, undefined, [shapeId, bucket]);
}

function kernelFloor(body: Solid): number {
  const buckets: unknown = JSON.parse(TESTING.graph().displayBuckets(body.ogId));
  const floor: unknown = typeof buckets === 'object' && buckets !== null ? Reflect.get(buckets, 'floor') : undefined;
  if (typeof floor !== 'number') throw new Error('Test page expected a display bucket floor');
  return floor;
}

function isRetryWarning(value: unknown): value is RetryWarning {
  return typeof value === 'object' && value !== null && typeof Reflect.get(value, 'code') === 'string'
    && typeof Reflect.get(value, 'bucket') === 'number' && typeof Reflect.get(value, 'retryBucket') === 'number';
}

export async function coarserRetryProbe(): Promise<Record<string, unknown>> {
  const body = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'retry-cube' });
  let warning: RetryWarning | undefined;
  const errors: unknown[] = [];
  const stopWarnings = OpenGeometry.on('warning', (event) => { if (isRetryWarning(event)) warning = event; });
  const stopErrors = OpenGeometry.on('error', (event) => { errors.push(event); });
  try {
    pinWantedBucket(required(body.lastInfo.shapeId, 'a shape id'), kernelFloor(body) / 2);
    TESTING.ensureGeometry(body);
    await OpenGeometry.settled();
    return { warning, bucket: body.record?.bucket, errors };
  } finally {
    stopWarnings();
    stopErrors();
    body.dispose();
  }
}
