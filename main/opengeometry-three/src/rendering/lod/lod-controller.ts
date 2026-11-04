import * as THREE from 'three';
import type { Body } from '../../bodies/body';
import { call } from '../../kernel/kernel-session';
import { EVALUATION_INTERVAL_MS, MOTION_WINDOW_MS, PIXELS_AT_REST, PIXELS_MOVING } from '../../limits';
import { currentRuntime, type Runtime } from '../../runtime/runtime-state';
import { finestLocal, localTarget, shapeBuckets, type ShapeBuckets } from './body-size';
import { deflectionBucket, wantedBucket } from './deflection';
import { displayBounds } from './display-bounds';
import { displayedBodies, isDisplayed, stampedCount } from './displayed';
import { groupsKey, queueShapes } from './host-groups';
import { worldUnitsPerPixel } from './screen-metrics';

type Tiers = { temporary: number | undefined; own: number | undefined; global: number | undefined };
type SeenCamera = { camera: THREE.Camera; height: number };
type ShapeTarget = { target: number; body: Body };
type TemporaryTarget = { deflection: number };
type PassInputs = { revision: bigint; flushed: bigint; moving: boolean; displayed: number; groups: number[] };

type ShapeLod = {
  shapeId: string;
  target?: number;
  previous?: number;
  failed: Set<string>;
  pinned?: { revision: number; bucket: number };
  forced?: number;
  error?: { revision: number; error: unknown };
  overrides?: { pass: number; tiers: Tiers };
};

export type LodState = {
  shapes: Map<string, ShapeLod>;
  camera: Float64Array;
  frames: WeakMap<THREE.WebGLRenderer, number>;
  bufferSize: THREE.Vector2;
  initialised: boolean;
  lastMotion: number;
  lastSchedule: number;
  settleTimer?: ReturnType<typeof setTimeout>;
  lastSeen?: SeenCamera;
  temporaries: TemporaryTarget[];
  stale: boolean;
  evaluated?: PassInputs;
  drawn: Set<Body>;
  walked: WeakMap<Body, number>;
  refresh: () => void;
};

export function createLodState(refresh: () => void): LodState {
  return {
    shapes: new Map(), camera: new Float64Array(33), frames: new WeakMap(), bufferSize: new THREE.Vector2(),
    initialised: false, lastMotion: -Infinity, lastSchedule: -Infinity, temporaries: [], stale: true, drawn: new Set(),
    walked: new WeakMap(), refresh,
  };
}

export function observe(state: Runtime, camera: THREE.Camera, height: number, now: number): void {
  const lod = state.lod;
  const moved = storeChanged(lod.camera, camera.matrixWorld.elements, 0);
  const projected = storeChanged(lod.camera, camera.projectionMatrix.elements, 16);
  const resized = lod.camera[32] !== height;
  lod.camera[32] = height;
  if (lod.initialised && (moved || projected || resized)) motionHint(state, now);
  lod.initialised = true;
  lod.lastSeen = { camera, height };
}

function storeChanged(target: Float64Array, values: readonly number[], offset: number): boolean {
  const same = values.every((value, index) => target[offset + index] === value);
  if (!same) target.set(values, offset);
  return !same;
}

export function motionHint(state: Runtime, now: number): void {
  const lod = state.lod;
  lod.lastMotion = now;
  lod.stale = true;
  clearTimeout(lod.settleTimer);
  lod.settleTimer = setTimeout(() => { fireSettle(state); }, MOTION_WINDOW_MS);
}

function fireSettle(state: Runtime): void {
  delete state.lod.settleTimer;
  if (currentRuntime() !== state || state.poisoned) return;
  if (evaluatePass(state, Date.now(), true)) state.lod.refresh();
}

export function settleNow(state: Runtime): void {
  clearTimeout(state.lod.settleTimer);
  delete state.lod.settleTimer;
  evaluatePass(state, Date.now(), true);
}

export function evaluatePass(state: Runtime, now: number, forced: boolean): boolean {
  const lod = state.lod;
  const seen = lod.lastSeen;
  if (!seen || state.poisoned || (!forced && now - lod.lastSchedule < EVALUATION_INTERVAL_MS)) return false;
  lod.lastSchedule = now;
  const moving = !forced && now - lod.lastMotion < MOTION_WINDOW_MS;
  const revision = call('OpenGeometry.flush', () => state.graph.revision());
  const groups = [state.groups.revision, groupsKey(state)];
  const inputs = { revision, flushed: state.revision, moving, displayed: stampedCount(state), groups };
  if (!forced && !stalePass(lod, inputs)) return false;
  const bodies = displayedBodies(state);
  lod.stale = false;
  lod.evaluated = inputs;
  let changed = false;
  const pixels = moving ? PIXELS_MOVING : PIXELS_AT_REST;
  for (const [shapeId, { target, body }] of shapeTargets(state, seen, bodies, inputs.revision, pixels)) {
    const buckets = shapeBuckets(state, body);
    if (!buckets) continue;
    const entry = shapeLod(state, shapeId);
    const previous = entry.previous;
    entry.target = target;
    entry.previous = wantedBucket(target, buckets.floor, previous, moving);
    changed ||= entry.previous !== previous;
  }
  return changed;
}

function stalePass(lod: LodState, inputs: PassInputs): boolean {
  const last = lod.evaluated;
  return lod.stale || !last || last.revision !== inputs.revision || last.flushed !== inputs.flushed
    || last.moving !== inputs.moving || last.displayed !== inputs.displayed
    || last.groups.some((value, index) => value !== inputs.groups[index]);
}

function shapeTargets(
  state: Runtime, seen: SeenCamera, bodies: Body[], revision: bigint, pixels: number,
): Map<string, ShapeTarget> {
  const targets = new Map<string, ShapeTarget>();
  for (const body of bodies) {
    const shapeId = body.lastInfo.shapeId;
    const bounds = shapeId ? displayBounds(state, body, revision) : null;
    const world = bounds ? worldUnitsPerPixel(seen.camera, bounds, seen.height) * pixels : undefined;
    const target = world === undefined ? undefined : localTarget(world, body);
    if (!shapeId || target === undefined || !Number.isFinite(target) || target <= 0) continue;
    const best = targets.get(shapeId);
    if (!best || target < best.target) targets.set(shapeId, { target, body });
  }
  return targets;
}

export function wantedKey(state: Runtime, body: Body): number | undefined {
  const { shapeId, shapeRevision: revision } = body.lastInfo;
  if (!shapeId || revision === null) return undefined;
  const buckets = shapeBuckets(state, body);
  if (!buckets) return undefined;
  const entry = shapeLod(state, shapeId);
  const forced = entry.forced;
  if (forced !== undefined) {
    delete entry.forced;
    return forced;
  }
  if (entry.pinned?.revision === revision) return entry.pinned.bucket;
  let bucket = chosenBucket(state, body, entry, buckets);
  while (entry.failed.has(failedKey(revision, bucket))) bucket *= 2;
  return bucket;
}

function chosenBucket(state: Runtime, body: Body, entry: ShapeLod, buckets: ShapeBuckets): number {
  const override = overrideTarget(state, body, entry);
  if (override !== undefined) return Math.max(buckets.floor, deflectionBucket(override));
  if (entry.previous !== undefined) return Math.max(buckets.floor, entry.previous);
  return buckets.static;
}

function overrideTarget(state: Runtime, body: Body, entry: ShapeLod): number | undefined {
  if (entry.overrides?.pass !== state.pass) {
    const members = [...(state.byShape.get(entry.shapeId) ?? [])].filter((member) => isDisplayed(state, member));
    entry.overrides = { pass: state.pass, tiers: tiers(state, members) };
  }
  const shape = entry.overrides.tiers;
  const own = tiers(state, [body]);
  return finer(shape.temporary, own.temporary) ?? finer(shape.own, own.own) ?? finer(shape.global, own.global);
}

function tiers(state: Runtime, bodies: Body[]): Tiers {
  return {
    temporary: finestLocal(bodies, () => state.lod.temporaries.at(-1)?.deflection),
    own: finestLocal(bodies, (body) => body.appearance.deflection),
    global: finestLocal(bodies, () => state.displayDeflection),
  };
}

function finer(first: number | undefined, second: number | undefined): number | undefined {
  if (first === undefined) return second;
  return second === undefined ? first : Math.min(first, second);
}

function shapeLod(state: Runtime, shapeId: string): ShapeLod {
  let entry = state.lod.shapes.get(shapeId);
  if (!entry) {
    entry = { shapeId, failed: new Set() };
    state.lod.shapes.set(shapeId, entry);
  }
  return entry;
}

function failedKey(revision: number, bucket: number): string {
  return `${String(revision)}#${String(bucket)}`;
}

export function overrideChanged(state: Runtime, shapeId?: string): void {
  state.lod.stale = true;
  queueShapes(state, shapeId);
  const entries = shapeId === undefined ? [...state.lod.shapes.values()] : [state.lod.shapes.get(shapeId)];
  for (const entry of entries) {
    if (!entry) continue;
    clearMarks(entry);
    delete entry.previous;
  }
}

export function pushTemporaryTarget(state: Runtime, deflection: number): TemporaryTarget {
  const token = { deflection };
  state.lod.temporaries.push(token);
  clearAllMarks(state);
  return token;
}

export function popTemporaryTarget(state: Runtime, token: TemporaryTarget): void {
  const temporaries = state.lod.temporaries;
  temporaries.splice(temporaries.indexOf(token), 1);
  clearAllMarks(state);
}

function clearAllMarks(state: Runtime): void {
  state.lod.stale = true;
  queueShapes(state);
  for (const entry of state.lod.shapes.values()) clearMarks(entry);
}

function clearMarks(entry: ShapeLod): void {
  entry.failed.clear();
  delete entry.pinned;
  delete entry.overrides;
}

export function markFailed(state: Runtime, shapeId: string, revision: number, bucket: number): boolean {
  const entry = shapeLod(state, shapeId);
  const key = failedKey(revision, bucket);
  if (entry.failed.has(key)) return false;
  entry.failed.add(key);
  entry.pinned = { revision, bucket: bucket * 2 };
  state.lod.stale = true;
  return true;
}

export function retryBucket(state: Runtime, shapeId: string, revision: number): number | undefined {
  const pinned = state.lod.shapes.get(shapeId)?.pinned;
  return pinned?.revision === revision ? pinned.bucket : undefined;
}

export function shapePriority(state: Runtime, shapeId: string): number {
  return state.lod.shapes.get(shapeId)?.target ?? 0;
}

export function recordError(state: Runtime, shapeId: string, revision: number, error: unknown): void {
  shapeLod(state, shapeId).error = { revision, error };
}

export function clearError(state: Runtime, shapeId: string, revision: number): void {
  const entry = state.lod.shapes.get(shapeId);
  if (entry?.error?.revision === revision) delete entry.error;
}

export function capturedError(state: Runtime, body: Body): unknown {
  const { shapeId, shapeRevision } = body.lastInfo;
  const failure = shapeId ? state.lod.shapes.get(shapeId)?.error : undefined;
  if (failure?.revision === shapeRevision) return failure.error;
  const buckets = shapeId ? state.shapeBuckets.get(shapeId) : undefined;
  return buckets?.revision === shapeRevision ? buckets.error : undefined;
}

export function forceNextBucket(state: Runtime, shapeId: string, bucket: number): void {
  shapeLod(state, shapeId).forced = bucket;
  state.lod.stale = true;
  queueShapes(state, shapeId);
}
