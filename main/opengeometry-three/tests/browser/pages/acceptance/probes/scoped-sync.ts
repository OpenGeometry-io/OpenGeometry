import * as THREE from 'three';
import { OpenGeometry, Solid, OG_PRIMITIVE_CUBOID, OG_TRANSFORM_TRANSLATE } from '../../../../../../dist/index.js';
import { activeBackend, graph } from '../../../../../../dist/testing.js';
import type { ScopedSyncResult } from '../../../support/fixture-types';
import type { AcceptancePage } from '../../acceptance-probes';

const PENDING_BODIES = 300;

function box(index: number): Solid {
  const origin: [number, number, number] = [(index % 20) * 2 - 20, -6, Math.floor(index / 20) * 2 - 20];
  return new Solid(OG_PRIMITIVE_CUBOID, { width: 1 + index / 1000, height: 1, depth: 1 }, {
    plane: { origin, normal: [0, 0, 1], xDirection: [1, 0, 0] },
  });
}

function placed(body: Solid, mesh: THREE.Object3D): boolean {
  const origin = body.record?.origin;
  if (!origin) return false;
  const expected = body.matrixWorld.clone().multiply(new THREE.Matrix4().makeTranslation(origin));
  return mesh.matrixWorld.elements.every((value, index) => Math.abs(value - (expected.elements[index] ?? 0)) < 1e-9);
}

function tessellated(step: () => void): string[] {
  const target: object = graph();
  const buffers: unknown = Reflect.get(target, 'buffers');
  if (typeof buffers !== 'function') throw new Error('graph.buffers is missing');
  const shapes: string[] = [];
  Reflect.set(target, 'buffers', (shapeId: string, ...rest: unknown[]): unknown => {
    shapes.push(shapeId);
    const result: unknown = Reflect.apply(buffers, target, [shapeId, ...rest]);
    return result;
  });
  try {
    step();
  } finally {
    Reflect.deleteProperty(target, 'buffers');
  }
  return shapes;
}

export async function scopedSyncProbe(page: AcceptancePage): Promise<ScopedSyncResult> {
  const { renderer, scene, camera } = page;
  const group = new THREE.Group();
  const bodies = Array.from({ length: PENDING_BODIES }, (_, index) => box(index));
  group.add(...bodies);
  scene.add(group);
  try {
    renderer.render(scene, camera);
    const [edited, ...others] = bodies;
    if (!edited) throw new Error('no edited body');
    edited.rebuild(OG_PRIMITIVE_CUBOID, { width: 3, height: 1, depth: 1 });
    edited.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 1, 0] });
    const before = others.map((body) => body.record);
    const shapes = tessellated(() => {
      const flush: unknown = Reflect.get(OpenGeometry, 'flush');
      if (typeof flush !== 'function') throw new Error('OpenGeometry.flush is missing');
      Reflect.apply(flush, OpenGeometry, [{ geometry: 'sync', bodies: [edited] }]);
    });
    const shapeId = edited.lastInfo.shapeId;
    return {
      backend: activeBackend(),
      pendingBefore: before.filter((record) => record === undefined).length,
      tessellated: shapes.length,
      onlyGiven: shapes.every((shape) => shape === shapeId),
      current: edited.record?.shapeId === shapeId && edited.record.revision === edited.lastInfo.shapeRevision,
      othersChanged: others.filter((body, index) => body.record !== before[index]).length,
      placed: placed(edited, edited.surface) && placed(edited, edited.outline),
    };
  } finally {
    for (const body of bodies) body.dispose();
    scene.remove(group);
    await OpenGeometry.settled();
  }
}
