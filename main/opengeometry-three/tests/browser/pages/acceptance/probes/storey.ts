import * as THREE from 'three';
import {
  OpenGeometry, SystemAssembly, Wire, Solid,
  OG_PRIMITIVE_RECTANGLE, OG_PRIMITIVE_CUBOID, OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT, OG_TRANSFORM_TRANSLATE,
} from '../../../../../../dist/index.js';
import { flushCount } from '../../../../../../dist/testing.js';
import type { StoreyPerformance, StoreyResult } from '../../../support/fixture-types';
import { required } from '../../../support/test-page';
import type { AcceptancePage } from '../../acceptance-probes';

type StoreyRender = Pick<StoreyResult, 'renderMs' | 'renderFlushes' | 'geometryCount' | 'failed'>;

const WALLS = 200;
const RAIL_INSTANCES = 1000;
const OPENING = { width: 0.6, height: 2.0, depth: 0.4 };

let storeyData: { root: SystemAssembly; bodies: Solid[] } | undefined;

function nextFrame(): Promise<void> {
  return new Promise<void>((resolve) => requestAnimationFrame(() => { resolve(); }));
}

function grid(index: number, columns: number): [number, number, number] {
  return [(index % columns) * 8, 0, Math.floor(index / columns) * 8];
}

function buildWall(index: number, members: (Wire | Solid)[]): Solid {
  const name = String(index);
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 6, breadth: 0.2 }, { ogId: `storey-profile-${name}` });
  const wall = new Solid(OG_OPERATION_EXTRUDE, { profile, distance: 3 }, { ogId: `storey-wall-${name}` });
  const cutters: Solid[] = [];
  for (let opening = 0; opening < 4; opening++) {
    const cutter = new Solid(OG_PRIMITIVE_CUBOID, OPENING, { ogId: `storey-opening-${name}-${String(opening)}` });
    cutter.transform(OG_TRANSFORM_TRANSLATE, { offset: [-2.1 + 1.4 * opening, 0, 0] });
    cutters.push(cutter);
  }
  wall.operate(OG_OPERATION_SUBTRACT, { tools: cutters });
  for (const cutter of cutters) cutter.dispose();
  wall.transform(OG_TRANSFORM_TRANSLATE, { offset: grid(index, 20) });
  members.push(profile, wall);
  return wall;
}

async function buildWalls(members: (Wire | Solid)[]): Promise<Solid[]> {
  const walls: Solid[] = [];
  for (let index = 0; index < WALLS; index++) {
    walls.push(buildWall(index, members));
    if (index % 10 === 9) await nextFrame();
  }
  return walls;
}

async function buildRails(rail: Solid): Promise<Solid[]> {
  const rails: Solid[] = [];
  for (let index = 0; index < RAIL_INSTANCES; index++) {
    const copy = rail.instance({ ogId: `storey-rail-${String(index)}` });
    copy.transform(OG_TRANSFORM_TRANSLATE, { offset: grid(index, 40) });
    rails.push(copy);
    if (index % 50 === 0) await nextFrame();
  }
  return rails;
}

function openingCount(wall: Solid): number {
  const cuts = wall.getBrep().topology.faces.filter((face) => face.provenance.role === 'Cut');
  return new Set(cuts.flatMap((face) => face.provenance.sources.map((source) => source.body))).size;
}

function openingRange(walls: Solid[]): { min: number; max: number } {
  const counts = walls.map(openingCount);
  return { min: Math.min(...counts), max: Math.max(...counts) };
}

function storeyCamera(bodies: Solid[]): THREE.PerspectiveCamera {
  const box = new THREE.Box3();
  for (const body of bodies) {
    box.union(new THREE.Box3().setFromArray(required(body.getBounds(), `${body.ogId} bounds`)));
  }
  const sphere = box.getBoundingSphere(new THREE.Sphere());
  const camera = new THREE.PerspectiveCamera(50, 640 / 480);
  const distance = sphere.radius / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2));
  camera.position.copy(sphere.center).addScaledVector(new THREE.Vector3(1, 1.2, 1).normalize(), distance);
  camera.near = (distance - sphere.radius) * 0.9;
  camera.far = (distance + sphere.radius) * 1.1;
  camera.lookAt(sphere.center);
  camera.updateProjectionMatrix();
  camera.updateMatrixWorld();
  return camera;
}

async function renderStorey(page: AcceptancePage, camera: THREE.PerspectiveCamera): Promise<StoreyRender> {
  const beforeRenderFlush = flushCount();
  const beforeRender = performance.now();
  page.renderer.render(page.scene, camera);
  const renderMs = performance.now() - beforeRender;
  const renderFlushes = flushCount() - beforeRenderFlush;
  const { failed } = await OpenGeometry.settled();
  page.renderer.render(page.scene, camera);
  return { renderMs, renderFlushes, geometryCount: page.renderer.info.memory.geometries, failed: failed.length };
}

export async function buildStorey(page: AcceptancePage): Promise<StoreyResult> {
  const storey = new SystemAssembly({ ogId: 'storey' });
  const members: (Wire | Solid)[] = [];
  const walls = await buildWalls(members);
  const rails = await buildRails(page.rail);
  storey.addChild([...members, ...rails]);
  storeyData = { root: storey, bodies: [...walls, ...rails] };
  page.scene.add(...walls, ...rails);
  const rendered = await renderStorey(page, storeyCamera(storeyData.bodies));
  return {
    walls: walls.length, uniqueWalls: walls.filter((wall) => wall.getInstanceCount() === 1).length,
    openingsPerWall: openingRange(walls), instances: rails.length, flushCount: flushCount(), ...rendered,
  };
}

export async function storeyPerformanceProbe(): Promise<StoreyPerformance> {
  if (!storeyData) throw new Error('buildStorey must run first');
  const startTransform = performance.now();
  storeyData.root.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 0, 0] });
  const transformMs = performance.now() - startTransform;
  const startFlush = performance.now();
  OpenGeometry.flush();
  const flushMs = performance.now() - startFlush;
  const transformFlushMs = transformMs + flushMs;
  const startStep = performance.now();
  const exported = await OpenGeometry.exportStep({ nodes: storeyData.bodies });
  const stepMs = performance.now() - startStep;
  return {
    transformFlushMs, transformMs, flushMs, stepMs, bytes: exported.text.length,
    products: exported.report.products, entities: exported.report.entities,
  };
}
