import * as THREE from 'three';
import {
  OpenGeometry, SystemAssembly, Wire, Solid, OGError,
  OG_PRIMITIVE_RECTANGLE, OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_POLYLINE, OG_PRIMITIVE_CIRCLE,
  OG_OPERATION_EXTRUDE, OG_OPERATION_SWEEP, OG_OPERATION_SUBTRACT,
  OG_TRANSFORM_TRANSLATE, OG_TRANSFORM_ROTATE, OG_TRANSFORM_PLACE,
} from '../../../../dist/index.js';
import { activeBackend, graph } from '../../../../dist/testing.js';
import {
  bootKernel, createRenderer, publishFixture, releasePage, required, statusElement,
} from '../support/test-page.js';
import {
  buildStorey, coarserRetryProbe, lodHysteresisProbe, lodProbe, memoryProbe, onDemandProbe, orbitProbe,
  placementPixelProbe, snapshotResendProbe, staleWorkerProbe, storeyPerformanceProbe, transactionProbe,
  workerCrashProbe,
} from './acceptance-probes.js';

type Point = [number, number, number];

window.addEventListener('error', (event) => { statusElement().textContent = `Error: ${event.message}`; });
window.addEventListener('unhandledrejection', (event) => {
  statusElement().textContent = `Error: ${String(event.reason)}`;
});

await bootKernel();
const SCENE = new THREE.Scene();
SCENE.background = new THREE.Color(0xf7f8fb);
const CAMERA = new THREE.PerspectiveCamera(50, 640 / 480, 0.1, 1000);
CAMERA.position.set(9, 9, 14);
CAMERA.lookAt(2, 3, 1);
const RENDERER = createRenderer(640, 480);
const BODIES: Solid[] = [];

function assertNear(actual: number, expected: number, tolerance: number, label: string): void {
  if (Math.abs(actual - expected) > tolerance) {
    throw new Error(`${label}: ${String(actual)} expected ${String(expected)}`);
  }
}

function meshValue(values: ArrayLike<number>, index: number): number {
  return required(values[index], `mesh value ${String(index)}`);
}

function volume(body: Solid, bucket = 0.01): number {
  const info = JSON.parse(graph().node(body.ogId)) as { shapeId: string };
  const buffers = graph().buffers(info.shapeId, bucket, 2_000_000) as {
    positions: Float32Array; indices: Uint32Array; origin: Float64Array;
  };
  let sum = 0;
  for (let i = 0; i < buffers.indices.length; i += 3) {
    const a = meshValue(buffers.indices, i) * 3;
    const b = meshValue(buffers.indices, i + 1) * 3;
    const c = meshValue(buffers.indices, i + 2) * 3;
    const point = (index: number): Point => [
      meshValue(buffers.positions, index) + meshValue(buffers.origin, 0),
      meshValue(buffers.positions, index + 1) + meshValue(buffers.origin, 1),
      meshValue(buffers.positions, index + 2) + meshValue(buffers.origin, 2),
    ];
    const pointA = point(a); const pointB = point(b); const pointC = point(c);
    sum += pointA[0] * (pointB[1] * pointC[2] - pointB[2] * pointC[1])
      + pointA[1] * (pointB[2] * pointC[0] - pointB[0] * pointC[2])
      + pointA[2] * (pointB[0] * pointC[1] - pointB[1] * pointC[0]);
  }
  return Math.abs(sum / 6);
}

const LEVEL = new SystemAssembly({ ogId: 'level-1' });
const PROFILE = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 6, breadth: 0.2 }, { ogId: 'wall-profile' });
const WALL = new Solid(OG_OPERATION_EXTRUDE, { profile: PROFILE, distance: 3 }, { ogId: 'wall-1' });
const CUTTER = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.9, height: 2.1, depth: 0.4 }, { ogId: 'door-cutter' });
assertNear(volume(WALL), 3.6, 1e-5, 'wall base volume');
CUTTER.transform(OG_TRANSFORM_TRANSLATE, { offset: [2, 0, 0] });
WALL.operate(OG_OPERATION_SUBTRACT, { tools: [CUTTER] });
assertNear(volume(WALL), 3.222, 1e-5, 'wall cut volume');
const PATH = new Wire(
  OG_PRIMITIVE_POLYLINE, { points: [[0, 1, 1], [4, 1, 1], [4, 1, 4]], closed: false }, { ogId: 'rail-path' },
);
const DISC = new Wire(OG_PRIMITIVE_CIRCLE, { radius: 0.05 }, {
  ogId: 'rail-disc', plane: { origin: [0, 1, 1], normal: [1, 0, 0], xDirection: [0, 0, 1] },
});
const RAIL = new Solid(OG_OPERATION_SWEEP, { profile: DISC, path: PATH }, { ogId: 'rail-1' });
assertNear(volume(RAIL, 0.002), Math.PI * 0.05 ** 2 * 7, 0.003, 'swept rail volume');
LEVEL.addChild([PROFILE, WALL, CUTTER, PATH, DISC, RAIL]);
SCENE.add(WALL, RAIL);
BODIES.push(WALL, RAIL);
const SHAPE_REVISIONS = [WALL.getBrep().revision, RAIL.getBrep().revision];
LEVEL.transform(OG_TRANSFORM_PLACE, { origin: [0, 3, 0] });
RAIL.transform(OG_TRANSFORM_ROTATE, { axis: [0, 1, 0], degrees: 90 });
if (WALL.getBrep().revision !== SHAPE_REVISIONS[0] || RAIL.getBrep().revision !== SHAPE_REVISIONS[1]) {
  throw new Error('Placement changed a shape revision');
}
const MOVED_BOUNDS = required(WALL.getBounds(), 'wall bounds');
assertNear(MOVED_BOUNDS[1], 3, 1e-6, 'moved wall low y');
WALL.rebuild(OG_OPERATION_EXTRUDE, { profile: PROFILE, distance: 4 });
assertNear(volume(WALL), 4.8, 1e-5, 'wall rebuild volume');
WALL.operate(OG_OPERATION_SUBTRACT, { tools: [CUTTER] });
assertNear(volume(WALL), 4.422, 1e-5, 'wall recut volume');
let coverageGap = false;
try {
  RAIL.operate(OG_OPERATION_SUBTRACT, { tools: [WALL] });
} catch (error) { coverageGap = error instanceof OGError && error.code === 'CoverageGap'; }
if (!coverageGap) throw new Error('CoverageGap pair did not return CoverageGap');
const BEFORE_TRIAL = JSON.stringify(WALL.getBrep());
const MARK = OpenGeometry.mark();
WALL.rebuild(OG_OPERATION_EXTRUDE, { profile: PROFILE, distance: 5 });
MARK.rollback();
MARK.release();
if (JSON.stringify(WALL.getBrep()) !== BEFORE_TRIAL) throw new Error('Mark rollback changed the wall');
const RAILS: Solid[] = [RAIL];
for (let i = 2; i <= 11; i++) {
  const copy = RAIL.instance({ ogId: `rail-${String(i)}` });
  copy.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 0, i * 2] });
  RAILS.push(copy);
  BODIES.push(copy);
  SCENE.add(copy);
}
const REPARENT = new SystemAssembly({ ogId: 'reparent' });
const TEST_CHILD = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.1, height: 0.1, depth: 0.1 }, { ogId: 'reparent-test' });
const BEFORE_KEEP = required(TEST_CHILD.getBounds(), 'child bounds');
REPARENT.transform(OG_TRANSFORM_TRANSLATE, { offset: [10, 0, 0] });
REPARENT.addChild([TEST_CHILD], { keepWorld: true });
assertNear(required(TEST_CHILD.getBounds(), 'child bounds')[0], BEFORE_KEEP[0], 1e-6, 'keepWorld');
REPARENT.removeChild(TEST_CHILD, { keepWorld: true });
REPARENT.addChild([TEST_CHILD]);
assertNear(required(TEST_CHILD.getBounds(), 'child bounds')[0], BEFORE_KEEP[0] + 10, 1e-6, 'default local reparent');
TEST_CHILD.dispose();
CUTTER.dispose();
RENDERER.render(SCENE, CAMERA);
await OpenGeometry.settled();
const FIRST_EXPORT = await OpenGeometry.exportStep({ nodes: [LEVEL] });
const SECOND_EXPORT = await OpenGeometry.exportStep({ nodes: [LEVEL] });
if (FIRST_EXPORT.text !== SECOND_EXPORT.text || FIRST_EXPORT.report.products !== 12) {
  throw new Error('Level STEP export is not deterministic');
}
const SKIPPED = FIRST_EXPORT.report.skipped as { ogId: string }[];
const SKIPPED_WIRES = ['wall-profile', 'rail-path', 'rail-disc'];
if (SKIPPED.length !== 3 || !SKIPPED_WIRES.every((id) => SKIPPED.some((item) => item.ogId === id))) {
  throw new Error('STEP wire skips differ');
}
if (FIRST_EXPORT.report.pcurvelessEdges <= 0) throw new Error('Swept rail lost pcurve-less edges');
const RESULT = {
  baseVolume: 3.6, cutVolume: 3.222, railVolume: Math.PI * 0.05 ** 2 * 7, rebuiltVolume: 4.8, recutVolume: 4.422,
  coverageGap, products: FIRST_EXPORT.report.products, skipped: SKIPPED.length,
  pcurvelessEdges: FIRST_EXPORT.report.pcurvelessEdges, stepBytes: FIRST_EXPORT.text.length,
};
const PAGE = { renderer: RENDERER, scene: SCENE, camera: CAMERA, wall: WALL, rail: RAIL };

publishFixture({
  bodies: BODIES, renderer: RENDERER, scene: SCENE, camera: CAMERA, level: LEVEL, wall: WALL, rail: RAIL, rails: RAILS,
  result: RESULT,
  get backend() { return activeBackend(); },
  flush: () => { OpenGeometry.flush(); },
  settled: () => OpenGeometry.settled(),
  render: () => { RENDERER.render(SCENE, CAMERA); },
  exportStep: () => OpenGeometry.exportStep({ nodes: [LEVEL] }),
  buildStorey: () => buildStorey(PAGE),
  storeyPerformanceProbe,
  memoryProbe: () => memoryProbe(PAGE),
  lodProbe: () => lodProbe(PAGE),
  workerCrashProbe: () => workerCrashProbe(PAGE),
  placementPixelProbe: () => placementPixelProbe(PAGE),
  onDemandProbe: () => onDemandProbe(PAGE),
  staleWorkerProbe: () => staleWorkerProbe(PAGE),
  snapshotResendProbe: () => snapshotResendProbe(PAGE),
  lodHysteresisProbe,
  orbitProbe: () => orbitProbe(PAGE),
  coarserRetryProbe,
  transactionProbe,
  dispose: () => {
    for (const body of [...BODIES].reverse()) { try { body.dispose(); } catch { continue; } }
    releasePage(RENDERER);
  },
}, 'Acceptance ready');
