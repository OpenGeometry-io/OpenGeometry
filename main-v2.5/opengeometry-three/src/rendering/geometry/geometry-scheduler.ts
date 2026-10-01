import type { ChangeSet } from '../../dto/change-set';
import type { DisplayBuffers } from '../../dto/display-buffers';
import { OGError } from '../../errors';
import { MAX_TRIANGLES } from '../../limits';
import { call } from '../../kernel/kernel-session';
import type { Body } from '../../bodies/body';
import { emit } from '../../runtime/event-bus';
import { bodyKey, currentRuntime, reindexShape, runtime, type Runtime } from '../../runtime/runtime-state';
import { decodeChangeSet } from '../../world-graph/codec';
import {
  clearError, displayedBodies, isDisplayed, markFailed, recordError, retryBucket, shapePriority, wantedKey,
} from '../lod/lod-controller';
import type { TessellationRequest } from '../provider';
import { GeometryRecord } from '../records/geometry-record';
import { validateDisplayBuffers } from '../records/validate-display-buffers';

type GeometryTarget = { body: Body; shapeId: string; revision: number; bucket: number; key: string };

export function flush(options: { geometry?: 'sync' } = {}): void {
  const state = runtime();
  const revision = call('OpenGeometry.flush', () => state.graph.revision());
  if (!state.flushing
    && (revision !== state.revision || (!state.renderPassActive && state.readyVersion !== state.flushedReadyVersion))) {
    state.flushing = true;
    try {
      const packet = call('OpenGeometry.flush', (): unknown => state.graph.changesSince(state.revision));
      const { changes, matrices } = decodeChangeSet(packet, 'OpenGeometry.flush');
      state.revision = revision;
      applyChanges(state, changes, matrices);
      for (const body of state.bodies.values()) body.swapReadyRecord();
      purgeChanged(state, changes);
      state.flushedReadyVersion = state.readyVersion;
      state.flushes++;
    } finally {
      state.flushing = false;
    }
  }
  if (options.geometry === 'sync') {
    for (const body of displayedBodies(state)) ensureGeometry(body, true);
    if (state.readyVersion !== state.flushedReadyVersion) flush();
  }
}

function applyChanges(state: Runtime, changes: ChangeSet, matrices: Float64Array): void {
  let offset = 0;
  for (const change of [...changes.added, ...changes.changed]) {
    const body = state.bodies.get(bodyKey(change.handle, change.generation));
    if (body) {
      const previous = body.lastInfo.shapeId;
      body.lastInfo = { ...body.lastInfo, shapeId: change.shapeId, shapeRevision: change.shapeRevision };
      reindexShape(state, body, previous);
      body.applyWorldMatrix(matrices.subarray(offset, offset + 16));
    }
    offset += 16;
  }
  for (const change of changes.removed) state.bodies.get(bodyKey(change.handle, change.generation))?.hideForDispose();
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
  const record = state.records.get(target.shapeId, target.revision, target.bucket);
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
  if (!shapeId || revision === null || !body.visible) return undefined;
  const bucket = wantedKey(state, body);
  if (bucket === undefined) return undefined;
  return { body, shapeId, revision, bucket, key: pendingKey(shapeId, revision, bucket) };
}

function pendingKey(shapeId: string, revision: number, bucket: number): string {
  return `${shapeId}@${String(revision)}#${String(bucket)}`;
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
  const { shapeId, revision, bucket } = target;
  if (!(error instanceof OGError && error.code === 'LimitExceeded') || !markFailed(state, shapeId, revision, bucket)) {
    return undefined;
  }
  emit('warning', { code: 'LimitExceeded', shapeId, revision, bucket, retryBucket: bucket * 2 });
  return { ...target, bucket: bucket * 2, key: pendingKey(shapeId, revision, bucket * 2) };
}

function reportFailure(state: Runtime, target: GeometryTarget, error: unknown): void {
  recordError(state, target.shapeId, target.revision, error);
  emit('error', error);
}

function computed(state: Runtime, request: TessellationRequest): DisplayBuffers {
  if (state.provider.compute) return state.provider.compute(request);
  return validateDisplayBuffers(call('OpenGeometry.flush', (): unknown =>
    state.graph.buffers(request.shapeId, request.bucket, request.maxTriangles)));
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
  const wanted = wantedKey(state, body);
  if (wanted === undefined) return false;
  const shown = body.record?.shapeId === shapeId && body.record.revision === revision ? body.record.bucket : undefined;
  return shown === undefined || Math.abs(Math.log2(buffers.bucket / wanted)) < Math.abs(Math.log2(shown / wanted));
}

function requestFor(state: Runtime, target: GeometryTarget): TessellationRequest {
  const { shapeId, revision, bucket } = target;
  const generation = (state.generations.get(shapeId) ?? 0) + 1;
  state.generations.set(shapeId, generation);
  const priority = shapePriority(state, shapeId);
  return { shapeId, revision, bucket, priority, maxTriangles: MAX_TRIANGLES, generation };
}

function receive(state: Runtime, target: GeometryTarget, buffers: DisplayBuffers): void {
  const { shapeId, revision } = target;
  const holders = [...(state.byShape.get(shapeId) ?? [])];
  const current = holders.filter((body) => body.lastInfo.shapeRevision === revision);
  if (current.length === 0) return;
  const record = state.records.put(new GeometryRecord(shapeId, buffers));
  state.readyVersion++;
  clearError(state, shapeId, revision);
  if (state.renderPassActive) {
    for (const body of current) if (isDisplayed(state, body)) body.install(record);
  }
  const ogIds = holders.map((body) => body.ogId);
  emit('geometry', { shapeId, revision, bucket: buffers.bucket, ogIds, stats: { triangles: record.triangles } });
  if (!state.renderPassActive) flush();
  state.records.purge(shapeId, revision);
}
