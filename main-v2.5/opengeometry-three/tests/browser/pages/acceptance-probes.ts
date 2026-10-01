import * as THREE from 'three';
import {
  OpenGeometry, SystemAssembly, Wire, Solid, OGError,
  OG_PRIMITIVE_RECTANGLE, OG_PRIMITIVE_CUBOID, OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT, OG_TRANSFORM_TRANSLATE,
} from '../../../../dist/index.js';
import { activeBackend, flushCount, graph, postToWorker, runtime, workerSendLog } from '../../../../dist/testing.js';
import { recordedErrors, required } from '../support/test-page.js';

export type AcceptancePage = {
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  wall: Solid;
  rail: Solid;
};
type StoreyResult = {
  walls: number;
  openingsPerWall: number;
  instances: number;
  flushCount: number;
  renderFlushes: number;
  geometryCount: number;
  renderMs: number;
};

let storeyData: { root: SystemAssembly; bodies: Solid[] } | undefined;

export async function buildStorey(page: AcceptancePage): Promise<StoreyResult> {
  const storey = new SystemAssembly({ ogId: 'storey' });
  const storeyProfile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 6, breadth: 0.2 }, { ogId: 'storey-profile' });
  const prototype = new Solid(OG_OPERATION_EXTRUDE, { profile: storeyProfile, distance: 3 }, { ogId: 'storey-wall-0' });
  const tools: Solid[] = [];
  for (let i = 0; i < 4; i++) {
    const opening = { width: 0.6, height: 2.0, depth: 0.4 };
    const tool = new Solid(OG_PRIMITIVE_CUBOID, opening, { ogId: `storey-opening-${String(i)}` });
    tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [-2.1 + i * 1.4, 0, 0] });
    tools.push(tool);
  }
  prototype.operate(OG_OPERATION_SUBTRACT, { tools });
  storey.addChild([storeyProfile, prototype]);
  const walls: Solid[] = [prototype];
  const instances: Solid[] = [];
  for (let i = 1; i < 200; i++) {
    const copy = prototype.instance({ ogId: `storey-wall-${String(i)}` });
    copy.transform(OG_TRANSFORM_TRANSLATE, { offset: [(i % 20) * 8, 0, Math.floor(i / 20) * 8] });
    walls.push(copy);
    if (i % 50 === 0) await new Promise<void>((resolve) => requestAnimationFrame(() => { resolve(); }));
  }
  for (let i = 0; i < 1000; i++) {
    const copy = page.rail.instance({ ogId: `storey-rail-${String(i)}` });
    copy.transform(OG_TRANSFORM_TRANSLATE, { offset: [(i % 40) * 8, 0, Math.floor(i / 40) * 8] });
    instances.push(copy);
    if (i % 50 === 0) await new Promise<void>((resolve) => requestAnimationFrame(() => { resolve(); }));
  }
  storey.addChild([...walls.slice(1), ...instances]);
  storeyData = { root: storey, bodies: [...walls, ...instances] };
  page.scene.add(...walls, ...instances);
  const beforeRenderFlush = flushCount();
  const beforeRender = performance.now();
  page.renderer.render(page.scene, page.camera);
  const renderMs = performance.now() - beforeRender;
  const renderFlushes = flushCount() - beforeRenderFlush;
  await OpenGeometry.settled();
  return {
    walls: walls.length, openingsPerWall: 4, instances: instances.length, flushCount: flushCount(),
    renderFlushes, geometryCount: page.renderer.info.memory.geometries, renderMs,
  };
}

export async function storeyPerformanceProbe(): Promise<Record<string, number>> {
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

export async function memoryProbe(page: AcceptancePage): Promise<Record<string, unknown>> {
  const { renderer, scene, camera } = page;
  renderer.render(scene, camera);
  await OpenGeometry.settled();
  const baseline = renderer.info.memory.geometries;
  const target = new Solid(OG_PRIMITIVE_CUBOID, { width: 2, height: 2, depth: 2 }, { ogId: 'memory-target' });
  const tool = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.5, height: 0.5, depth: 0.5 }, { ogId: 'memory-tool' });
  tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [0.5, 0, 0] });
  scene.add(target);
  renderer.render(scene, camera);
  await OpenGeometry.settled();
  renderer.render(scene, camera);
  await OpenGeometry.settled();
  const one = renderer.info.memory.geometries;
  const copy = target.instance({ ogId: 'memory-instance' });
  scene.add(copy);
  renderer.render(scene, camera);
  await OpenGeometry.settled();
  const shared = renderer.info.memory.geometries;
  const sameGeometry = target.surface.geometry === copy.surface.geometry;
  const sharedDetails = { target: target.record?.key, copy: copy.record?.key, same: sameGeometry };
  copy.makeUnique();
  copy.operate(OG_OPERATION_SUBTRACT, { tools: [tool] });
  renderer.render(scene, camera);
  await OpenGeometry.settled();
  const unique = renderer.info.memory.geometries;
  copy.dispose();
  target.dispose();
  tool.dispose();
  renderer.render(scene, camera);
  const after = renderer.info.memory.geometries;
  return { baseline, one, shared, unique, after, sharedDetails };
}

function errorCodes(): string[] {
  return recordedErrors().map((event) => (event instanceof OGError ? event.code : String(event)));
}

export function tessellateSends(shapeId: string): number {
  return workerSendLog().filter((send) => send.shapeId === shapeId).length;
}

export async function workerCrashProbe(page: AcceptancePage): Promise<Record<string, unknown>> {
  const { renderer, scene, camera, wall } = page;
  const record = wall.record;
  await postToWorker({ kind: 'throw' }, { awaitCrash: true });
  const info = JSON.parse(graph().node(wall.ogId)) as { shapeId: string; shapeRevision: number };
  await runtime().provider.ensureSnapshot(info.shapeId, info.shapeRevision);
  const afterRestart = activeBackend();
  await postToWorker({ kind: 'throw' }, { awaitCrash: true });
  const errors = errorCodes();
  const afterFallback = activeBackend();
  const body = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.3, height: 0.3, depth: 0.3 }, { ogId: 'after-fallback' });
  scene.add(body);
  renderer.render(scene, camera);
  const drawnInSameRender = Boolean(body.record);
  body.dispose();
  return { afterRestart, afterFallback, sameRecord: wall.record === record, errors, drawnInSameRender };
}

export async function workerFailureProbe(page: AcceptancePage): Promise<Record<string, unknown>> {
  const { failed } = await OpenGeometry.settled();
  const hasRecord = Boolean(page.wall.record);
  return { failed: failed.length, hasRecord, errors: errorCodes(), backend: activeBackend() };
}

export function placementPixelProbe(page: AcceptancePage): Record<string, unknown> {
  const { renderer } = page;
  const isolated = new THREE.Scene();
  isolated.background = new THREE.Color(0xffffff);
  isolated.add(new THREE.HemisphereLight(0xffffff, 0x526070, 2));
  const light = new THREE.DirectionalLight(0xffffff, 2);
  light.position.set(5, 8, 5);
  isolated.add(light);
  const view = new THREE.OrthographicCamera(-2, 2, 1.5, -1.5, 0.1, 20);
  view.position.set(0, 1, 5);
  view.lookAt(0, 0.5, 0);
  const cube = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'pixel-cube' });
  isolated.add(cube);
  renderer.render(isolated, view);
  OpenGeometry.flush({ geometry: 'sync' });
  const ray = new THREE.Raycaster(new THREE.Vector3(0, 0.5, 5), new THREE.Vector3(0, 0, -1));
  const syncHit = ray.intersectObject(cube.surface).length > 0;
  renderer.render(isolated, view);
  const gl = renderer.getContext();
  const pixel = (): number[] => {
    const value = new Uint8Array(4);
    gl.readPixels(320, 240, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, value);
    return Array.from(value);
  };
  const before = pixel();
  const geometry = cube.surface.geometry;
  const position = geometry.getAttribute('position');
  if (!(position instanceof THREE.BufferAttribute)) throw new Error('Test page expected a position BufferAttribute');
  const version = position.version;
  const revision = cube.getBrep().revision;
  cube.transform(OG_TRANSFORM_TRANSLATE, { offset: [4, 0, 0] });
  renderer.render(isolated, view);
  const after = pixel();
  const kernel = graph().worldMatrix(cube.ogId);
  const matrixMatches = cube.matrixWorld.elements
    .every((value, index) => Math.abs(value - (kernel[index] ?? Number.NaN)) <= 1e-12);
  const geometryMatches = cube.surface.geometry === geometry && geometry.getAttribute('position') === position
    && position.version === version && cube.getBrep().revision === revision;
  cube.dispose();
  return { before, after, matrixMatches, geometryMatches, syncHit };
}

export async function onDemandProbe(page: AcceptancePage): Promise<{ events: number; appeared: boolean }> {
  const { renderer, scene, camera } = page;
  const body = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.4, height: 0.4, depth: 0.4 }, { ogId: 'on-demand' });
  scene.add(body);
  let events = 0;
  const ready = new Promise<void>((resolve) => {
    const unsubscribe = OpenGeometry.on('geometry', (event) => {
      const value = event as { ogIds: string[] };
      if (value.ogIds.includes(body.ogId)) {
        events++;
        unsubscribe();
        renderer.render(scene, camera);
        resolve();
      }
    });
  });
  renderer.render(scene, camera);
  await ready;
  const appeared = Boolean(body.record && body.surface.geometry.getAttribute('position'));
  body.dispose();
  return { events, appeared };
}

export async function staleWorkerProbe(page: AcceptancePage): Promise<Record<string, unknown>> {
  const { renderer, scene, camera } = page;
  const body = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'rapid-rebuild' });
  scene.add(body);
  renderer.render(scene, camera);
  await OpenGeometry.settled();
  const info = JSON.parse(graph().node(body.ogId)) as { shapeId: string };
  const before = tessellateSends(info.shapeId);
  for (let i = 0; i < 10; i++) {
    body.rebuild(OG_PRIMITIVE_CUBOID, { width: 1 + i * 0.1, height: 1, depth: 1 });
    renderer.render(scene, camera);
  }
  await OpenGeometry.settled();
  const after = tessellateSends(info.shapeId);
  const current = body.getBrep().revision;
  const displayed = body.record?.revision;
  body.dispose();
  return { current, displayed, jobs: after - before };
}

export async function snapshotResendProbe(page: AcceptancePage): Promise<{ triangles: number }> {
  const info = JSON.parse(graph().node(page.rail.ogId)) as { shapeId: string; shapeRevision: number };
  const provider = runtime().provider;
  await provider.ensureSnapshot(info.shapeId, info.shapeRevision);
  await postToWorker({ kind: 'drop', shapeId: info.shapeId, revision: info.shapeRevision });
  const buffers = await provider.request({
    shapeId: info.shapeId, revision: info.shapeRevision, bucket: required(page.rail.record, 'a rail record').bucket * 2,
    maxTriangles: 2_000_000, priority: 0, generation: 1,
  });
  return { triangles: buffers.triangles };
}

export function transactionProbe(): Record<string, unknown> {
  const body = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'transaction-cube' });
  const before = JSON.stringify(body.getBrep());
  const placement = JSON.stringify(body.getPlacement());
  try {
    let threw = false;
    try {
      OpenGeometry.transaction(() => {
        OpenGeometry.transaction(() => { body.dispose(); });
        throw new Error('outer rollback');
      });
    } catch (error) { threw = error instanceof Error && error.message === 'outer rollback'; }
    const revived = JSON.stringify(body.getBrep()) === before && !body.inLimbo;
    const dryResult = OpenGeometry.transaction(() => {
      body.transform(OG_TRANSFORM_TRANSLATE, { offset: [3, 0, 0] });
      return 'trial';
    }, { dryRun: true });
    const dryRestored = JSON.stringify(body.getPlacement()) === placement;
    let thenableCode = '';
    try { OpenGeometry.transaction(() => ({ then: () => undefined })); }
    catch (error) { if (error instanceof OGError) thenableCode = error.code; }
    return { threw, revived, dryResult, dryRestored, thenableCode };
  } finally { body.dispose(); }
}

export async function reuseProbe(page: AcceptancePage): Promise<Record<string, unknown>> {
  const { renderer, scene, camera } = page;
  const errors: unknown[] = [];
  const stopErrors = OpenGeometry.on('error', (event) => { errors.push(event); });
  const size = { width: 1, height: 1, depth: 1 };
  const first = new Solid(OG_PRIMITIVE_CUBOID, size, { ogId: 'reuse-cube' });
  try {
    scene.add(first);
    renderer.render(scene, camera);
    await OpenGeometry.settled();
    const mark = OpenGeometry.mark();
    first.dispose();
    const second = new Solid(OG_PRIMITIVE_CUBOID, size, { ogId: 'reuse-cube' });
    scene.add(second);
    renderer.render(scene, camera);
    await OpenGeometry.settled();
    mark.rollback();
    mark.release();
    renderer.render(scene, camera);
    await OpenGeometry.settled();
    const revived = first.parent === scene && !first.inLimbo && Boolean(first.record);
    return { revived, gone: second.parent === null && !second.inLimbo, errors };
  } finally {
    stopErrors();
    first.dispose();
  }
}
