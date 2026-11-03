import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE } from '../../../../dist/index.js';
import { runtime } from '../../../../dist/testing.js';
import type { ReleaseIdleResult } from '../support/fixture-types';
import { bootKernel, createRenderer, publishFixture } from '../support/test-page';

type PooledRecord = { bytes: number; surface: THREE.BufferGeometry | null; outline: THREE.BufferGeometry };

function isPooled(value: unknown): value is PooledRecord {
  return typeof value === 'object' && value !== null && 'bytes' in value && 'outline' in value;
}

function idleRecords(): PooledRecord[] {
  const idle: unknown = Reflect.get(runtime().records, 'idle');
  if (!(idle instanceof Set)) throw new Error('record pool has no idle set');
  return [...idle].filter(isPooled);
}

function disposeListeners(geometry: THREE.BufferGeometry | null): number {
  const listeners: unknown = geometry ? Reflect.get(geometry, '_listeners') : undefined;
  const dispose: unknown = listeners ? Reflect.get(listeners, 'dispose') : undefined;
  return Array.isArray(dispose) ? dispose.length : 0;
}

function withListeners(records: PooledRecord[]): number {
  return records.filter((record) => disposeListeners(record.surface) + disposeListeners(record.outline) > 0).length;
}

function sceneOf(count: number): THREE.Scene {
  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
  for (let index = 0; index < count; index++) {
    const body = new Solid(OG_PRIMITIVE_CUBOID, { width: 1 + index / 100, height: 1, depth: 1 });
    body.transform(OG_TRANSFORM_TRANSLATE, { offset: [(index % 5) * 2 - 4, 0, Math.floor(index / 5) * 2 - 4] });
    scene.add(body);
  }
  return scene;
}

function disposeReachable(scene: THREE.Scene): void {
  const owned = new Set<THREE.BufferGeometry | THREE.Material | THREE.Texture>();
  scene.traverse((object) => {
    const geometry: unknown = Reflect.get(object, 'geometry');
    if (geometry instanceof THREE.BufferGeometry) owned.add(geometry);
    const materials: unknown = Reflect.get(object, 'material');
    for (const material of [materials].flat()) {
      if (!(material instanceof THREE.Material)) continue;
      owned.add(material);
      for (const value of Object.values(material)) if (value instanceof THREE.Texture) owned.add(value);
    }
  });
  for (const item of owned) item.dispatchEvent({ type: 'dispose' });
}

async function drawnIdleRecords(scene: THREE.Scene, camera: THREE.Camera): Promise<void> {
  const renderer = createRenderer(320, 240);
  for (const deflection of [undefined, 0.5, 0.002]) {
    OpenGeometry.setDisplayDeflection(deflection);
    renderer.render(scene, camera);
    await OpenGeometry.settled();
    renderer.render(scene, camera);
  }
  disposeReachable(scene);
  renderer.dispose();
  renderer.forceContextLoss();
  renderer.domElement.remove();
}

async function releaseIdleProbe(): Promise<ReleaseIdleResult> {
  const scene = sceneOf(20);
  const camera = new THREE.PerspectiveCamera(60, 4 / 3, 0.1, 100);
  camera.position.set(6, 8, 12);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld();
  await drawnIdleRecords(scene, camera);
  const idle = idleRecords();
  const idleBytes = idle.reduce((sum, record) => sum + record.bytes, 0);
  const idleWithListeners = withListeners(idle);
  const released = OpenGeometry.releaseIdleGeometry();
  return {
    idle: idle.length, idleBytes, idleWithListeners, released, idleAfter: idleRecords().length,
    releasedWithListeners: withListeners(idle),
  };
}

await bootKernel();
publishFixture('ogReleaseIdle', { releaseIdleProbe }, 'Release idle ready');
