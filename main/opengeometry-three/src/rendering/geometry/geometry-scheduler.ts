import * as THREE from 'three';
import type { ChangeSet } from '../../dto/change-set';
import type { DisplayBuffers } from '../../dto/display-buffers';
import { OGError } from '../../errors';
import { MAX_TRIANGLES } from '../../limits';
import { call } from '../../kernel/kernel-session';
import type { Body } from '../../bodies/body';
import { emit } from '../../runtime/event-bus';
import { bodyKey, currentRuntime, reindexShape, runtime, type Runtime } from '../../runtime/runtime-state';
import { decodeChangeSet } from '../../world-graph/codec';
import { displayedBodies, isDisplayed, stampDisplayed } from '../lod/displayed';
import { shown } from '../lod/host-drawn';
import { queueGrouped } from '../lod/host-groups';
import { clearError, markFailed, recordError, retryBucket, shapePriority, wantedKey } from '../lod/lod-controller';
import type { TessellationRequest } from '../provider';
import { GeometryRecord } from '../records/geometry-record';
import { validateDisplayBuffers } from '../records/validate-display-buffers';

type GeometryTarget = {
  body: Body; shapeId: string; revision: number; bucket: number; smoothAngle: number | undefined; key: string;
};

export type FlushOptions = { geometry?: 'sync'; bodies?: readonly Body[] };

export function flush(options: FlushOptions = {}): void {
  const state = runtime();
  const given = options.bodies && liveBodies(state, options.bodies, options.geometry);
  applyFlush(state);
  if (options.geometry !== 'sync') return;
  for (const body of given ?? displayedBodies(state)) {
    if (given && (!shown(state, body) || !body.lastInfo.shapeId || body.lastInfo.shapeRevision === null)) continue;
    if (given) stampDisplayed(state, body);
    ensureGeometry(body, true);
  }
  if (state.readyShapes.size > 0) applyFlush(state);
}

function liveBodies(state: Runtime, bodies: readonly Body[], geometry: FlushOptions['geometry']): readonly Body[] {
  const label = 'OpenGeometry.flush';
  if (geometry !== 'sync') throw new OGError('InvalidParameter', label, "bodies needs geometry: 'sync'");
  return checkedBodies(state, bodies, label);
}

export function checkedBodies(
  state: Runtime, bodies: readonly Body[], label: string, skipDisposed = false,
): readonly Body[] {
  const list: unknown = bodies;
  if (!Array.isArray(list)) throw new OGError('InvalidParameter', label, 'bodies must be an array of bodies');
  const live: Body[] = [];
  for (const body of bodies) {
    const item: unknown = body;
    if (!(item instanceof THREE.Object3D) || !('swapReadyRecord' in item)) {
      throw new OGError('InvalidParameter', label, 'bodies must hold only Wire, Sheet or Solid bodies');
    }
    if (state.bodies.get(bodyKey(body.handle, body.generation)) === body) live.push(body);
    else if (!skipDisposed) throw new OGError('Disposed', label, `${body.ogId} is disposed`);
  }
  return live;
}

export function flushForDispose(): void {
  applyFlush(runtime());
}

export function notifyBody(body: Body, type: 'geometrychanged' | 'placementchanged'): void {
  try {
    body.dispatchEvent({ type });
  } catch (error) {
    emit('error', error);
  }
}

function applyFlush(state: Runtime): void {
  const revision = call('OpenGeometry.flush', () => state.graph.revision());
  state.graphMoved = false;
  if (!state.flushing && (revision !== state.revision || (!state.renderPassActive && state.readyShapes.size > 0))) {
    state.flushing = true;
    const moved: Body[] = [];
    try {
      const packet = call('OpenGeometry.flush', (): unknown => state.graph.changesSince(state.revision));
      const { changes, matrices } = decodeChangeSet(packet, 'OpenGeometry.flush');
      state.revision = revision;
      moved.push(...applyChanges(state, changes, matrices));
      for (const body of swapTargets(state, changes)) body.swapReadyRecord();
      purgeChanged(state, changes);
      state.flushes++;
    } finally {
      state.flushing = false;
    }
    for (const body of moved) notifyBody(body, 'placementchanged');
  }
}

function swapTargets(state: Runtime, changes: ChangeSet): Body[] {
  const targets = new Set<Body>();
  for (const change of [...changes.added, ...changes.changed]) {
    const body = state.bodies.get(bodyKey(change.handle, change.generation));
    if (body) targets.add(body);
  }
  for (const shapeId of state.readyShapes) for (const body of state.byShape.get(shapeId) ?? []) targets.add(body);
  state.readyShapes.clear();
  return [...targets].filter((body) => isDisplayed(state, body));
}

function applyChanges(state: Runtime, changes: ChangeSet, matrices: Float64Array): Body[] {
  const moved: Body[] = [];
  let offset = 0;
  for (const change of [...changes.added, ...changes.changed]) {
    const body = state.bodies.get(bodyKey(change.handle, change.generation));
    if (body) {
      const previous = body.lastInfo.shapeId;
      body.lastInfo = { ...body.lastInfo, shapeId: change.shapeId, shapeRevision: change.shapeRevision };
      reindexShape(state, body, previous);
      const matrix = matrices.subarray(offset, offset + 16);
      if (body.matrixWorld.elements.some((value, index) => value !== matrix[index])) moved.push(body);
      body.applyWorldMatrix(matrix);
      queueGrouped(state, body);
    }
    offset += 16;
  }
  for (const change of changes.removed) state.bodies.get(bodyKey(change.handle, change.generation))?.hideForDispose();
  return moved;
}

function purgeChanged(state: Runtime, changes: ChangeSet): void {
  const shapes = new Map<string, { shapeId: string; revision: number }>();
  for (const { shapeId, shapeRevision } of [...changes.added, ...changes.changed]) {
    if (shapeId !== null && shapeRevision !== null) {
      shapes.set(`${shapeId}@${String(shapeRevision)}`, { shapeId, revision: shapeRevision });
    }
  }
  for (const { shapeId, revision } of shapes.values()) state.records.purge(shapeId, revision);
}

export function wanted(body: Body): number | undefined {
  return wantedKey(runtime(), body);
}

export function ensureGeometry(body: Body, sync = false): void {
  const state = runtime();
  const target = geometryTarget(state, body);
  if (!target) return;
  const record = state.records.get(target.shapeId, target.revision, target.bucket, target.smoothAngle);
  if (record) {
    body.install(record);
    return;
  }
  if (sync || state.provider.compute) {
    computeNow(state, target, sync);
    return;
  }
  if (state.pending.has(target.key)) return;
  requestLater(state, target);
}

function geometryTarget(state: Runtime, body: Body): GeometryTarget | undefined {
  const { shapeId, shapeRevision: revision } = body.lastInfo;
  if (!shapeId || revision === null || !shown(state, body)) return undefined;
  const bucket = wantedKey(state, body);
  if (bucket === undefined) return undefined;
  const { smoothAngle } = body.appearance;
  return { body, shapeId, revision, bucket, smoothAngle, key: pendingKey(shapeId, revision, bucket, smoothAngle) };
}

function pendingKey(shapeId: string, revision: number, bucket: number, smoothAngle?: number): string {
  return `${shapeId}@${String(revision)}#${String(bucket)}~${String(smoothAngle)}`;
}

function computeNow(state: Runtime, target: GeometryTarget, sync: boolean): void {
  const first = computeFailure(state, target, sync);
  if (!first) return;
  const retry = coarserRetry(state, target, first.error);
  const failure = retry ? computeFailure(state, retry, false) : first;
  if (failure) reportFailure(state, target, failure.error);
}

function computeFailure(state: Runtime, target: GeometryTarget, sync: boolean): { error: unknown } | undefined {
  try {
    const request = requestFor(state, target);
    if (sync) state.provider.cancelShape(target.shapeId, request.generation);
    receive(state, target, computed(state, request));
    return undefined;
  } catch (error) {
    if (fatal(state, error)) throw error;
    return { error };
  }
}

function coarserRetry(state: Runtime, target: GeometryTarget, error: unknown): GeometryTarget | undefined {
  const { shapeId, revision, bucket, smoothAngle } = target;
  if (!(error instanceof OGError && error.code === 'LimitExceeded') || !markFailed(state, shapeId, revision, bucket)) {
    return undefined;
  }
  emit('warning', { code: 'LimitExceeded', shapeId, revision, bucket, retryBucket: bucket * 2 });
  return { ...target, bucket: bucket * 2, key: pendingKey(shapeId, revision, bucket * 2, smoothAngle) };
}

function reportFailure(state: Runtime, target: GeometryTarget, error: unknown): void {
  recordError(state, target.shapeId, target.revision, error);
  emit('error', error);
}

function computed(state: Runtime, request: TessellationRequest): DisplayBuffers {
  if (state.provider.compute) return state.provider.compute(request);
  return validateDisplayBuffers(call('OpenGeometry.flush', (): unknown =>
    state.graph.buffers(request.shapeId, request.bucket, request.maxTriangles, request.smoothAngle)));
}

function fatal(state: Runtime, error: unknown): boolean {
  return state.poisoned && error instanceof OGError && error.code === 'KernelPanic';
}

function requestLater(state: Runtime, target: GeometryTarget): void {
  let request = requestFor(state, target);
  const promise = state.provider.request(request).catch(async (error: unknown) => {
    const retry = currentRuntime() === state ? coarserRetry(state, target, error) : undefined;
    if (!retry) throw error;
    request = requestFor(state, retry);
    return state.provider.request(request);
  }).then((buffers) => {
    if (currentRuntime() === state && accepted(state, target, buffers, request.generation)) {
      receive(state, target, buffers);
    }
  }).catch((error: unknown) => {
    if (currentRuntime() !== state || fatal(state, error)) return;
    if (!(error instanceof OGError && error.code === 'Cancelled')) reportFailure(state, target, error);
  }).finally(() => { state.pending.delete(target.key); });
  state.pending.set(target.key, promise);
}

function accepted(state: Runtime, target: GeometryTarget, buffers: DisplayBuffers, generation: number): boolean {
  const { body, shapeId, revision } = target;
  if (generation === (state.generations.get(shapeId) ?? 0)) return true;
  if (buffers.bucket === retryBucket(state, shapeId, revision)) return true;
  const wanted = isDisplayed(state, body) ? wantedKey(state, body) : undefined;
  if (wanted === undefined) return false;
  const shown = body.record?.shapeId === shapeId && body.record.revision === revision ? body.record.bucket : undefined;
  return shown === undefined || Math.abs(Math.log2(buffers.bucket / wanted)) < Math.abs(Math.log2(shown / wanted));
}

function requestFor(state: Runtime, target: GeometryTarget): TessellationRequest {
  const { shapeId, revision, bucket, smoothAngle } = target;
  const generation = (state.generations.get(shapeId) ?? 0) + 1;
  state.generations.set(shapeId, generation);
  const priority = shapePriority(state, shapeId);
  return { shapeId, revision, bucket, priority, maxTriangles: MAX_TRIANGLES, smoothAngle, generation };
}

function receive(state: Runtime, target: GeometryTarget, buffers: DisplayBuffers): void {
  const { shapeId, revision } = target;
  const holders = [...(state.byShape.get(shapeId) ?? [])];
  const current = holders.filter((body) => body.lastInfo.shapeRevision === revision);
  if (current.length === 0) return;
  const record = state.records.put(new GeometryRecord(shapeId, buffers, target.smoothAngle));
  state.readyShapes.add(shapeId);
  clearError(state, shapeId, revision);
  if (state.renderPassActive) {
    for (const body of current) if (isDisplayed(state, body)) body.install(record);
  } else if (shown(state, target.body) && current.includes(target.body)) target.body.install(record);
  const ogIds = holders.map((body) => body.ogId);
  emit('geometry', { shapeId, revision, bucket: buffers.bucket, ogIds, stats: { triangles: record.triangles } });
  if (!state.renderPassActive) flush();
  state.records.purge(shapeId, revision);
}
