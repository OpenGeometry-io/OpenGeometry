import type { ChangeSet } from '../../dto/change-set.js';
import type { DisplayBuffers } from '../../dto/display-buffers.js';
import { OGError } from '../../errors.js';
import { call } from '../../kernel/kernel-session.js';
import type { Body } from '../../bodies/body.js';
import { emit } from '../../runtime/event-bus.js';
import { runtime, type Runtime } from '../../runtime/runtime-state.js';
import { node } from '../../world-graph/world-graph-client.js';
import { wantedBucket } from '../lod/deflection.js';
import type { TessellationRequest } from '../provider.js';
import { GeometryRecord } from '../records/geometry-record.js';

type GeometryTarget = { body: Body; shapeId: string; revision: number; bucket: number; key: string };

export function flush(options: { geometry?: 'sync' } = {}): void {
  const state = runtime();
  const revision = state.graph.revision();
  if (revision !== state.revision || (!state.renderPassActive && state.readyVersion !== state.flushedReadyVersion)) {
    const packet = call('OpenGeometry.flush', () => state.graph.changesSince(state.revision)) as {
      changesJson: string;
      matrices: Float64Array;
    };
    const changes = JSON.parse(packet.changesJson) as ChangeSet;
    let offset = 0;
    for (const changed of [...changes.added, ...changes.changed]) {
      const body = state.bodies.get(changed.ogId);
      if (body) {
        body.lastInfo = node(body.ogId);
        body.applyWorldMatrix(packet.matrices.subarray(offset, offset + 16));
      }
      offset += 16;
    }
    for (const removed of changes.removed) state.bodies.get(removed.ogId)?.hideForDispose();
    for (const body of state.bodies.values()) body.swapReadyRecord();
    for (const body of state.bodies.values()) {
      if (body.lastInfo.shapeId && body.lastInfo.shapeRevision !== null) {
        state.records.purge(body.lastInfo.shapeId, body.lastInfo.shapeRevision);
      }
    }
    state.revision = revision;
    state.flushedReadyVersion = state.readyVersion;
    state.flushes++;
  }
  if (options.geometry === 'sync') {
    for (const body of state.displayed) ensureGeometry(body, true);
    if (state.readyVersion !== state.flushedReadyVersion) flush();
  }
}

export function wanted(body: Body): number {
  const state = runtime();
  const info = body.lastInfo;
  if (!info.shapeId) return 0.01;
  const { diagonal, floor } = body.displaySize();
  const target = body.appearance.deflection ?? state.displayDeflection
    ?? state.cameraBuckets.get(info.shapeId) ?? Math.min(1, diagonal / 500);
  const previous = state.buckets.get(info.shapeId);
  const bucket = wantedBucket(Math.max(target, floor), floor, previous, state.moving);
  state.buckets.set(info.shapeId, bucket);
  return bucket;
}

export function ensureGeometry(body: Body, sync = false): void {
  const state = runtime();
  const info = body.lastInfo;
  if (!info.shapeId || info.shapeRevision === null || !body.visible) return;
  const bucket = wanted(body);
  const record = state.records.get(info.shapeId, info.shapeRevision, bucket);
  if (record) {
    body.install(record);
    return;
  }
  const key = `${info.shapeId}@${String(info.shapeRevision)}#${String(bucket)}`;
  const target = { body, shapeId: info.shapeId, revision: info.shapeRevision, bucket, key };
  if (sync || state.provider.compute) {
    computeNow(state, target, sync);
    return;
  }
  if (state.pending.has(key)) return;
  requestLater(state, target);
}

function computeNow(state: Runtime, target: GeometryTarget, sync: boolean): void {
  const { body, shapeId, revision, bucket, key } = target;
  try {
    const request = requestFor(body, bucket);
    if (sync) state.provider.cancelShape(shapeId, request.generation);
    const buffers = state.provider.compute
      ? state.provider.compute(request)
      : state.graph.buffers(shapeId, bucket, 2_000_000) as DisplayBuffers;
    receive(shapeId, revision, bucket, buffers);
  } catch (error) {
    if (error instanceof OGError && error.code === 'LimitExceeded' && !state.failedBuckets.has(key)) {
      state.failedBuckets.add(key);
      state.buckets.set(shapeId, bucket * 2);
      emit('warning', { code: 'LimitExceeded', shapeId, revision, bucket, retryBucket: bucket * 2 });
      try {
        const coarser = requestFor(body, bucket * 2);
        const buffers = state.provider.compute
          ? state.provider.compute(coarser)
          : state.graph.buffers(shapeId, bucket * 2, 2_000_000) as DisplayBuffers;
        receive(shapeId, revision, bucket * 2, buffers);
      } catch (retry) { emit('error', retry); }
    } else emit('error', error);
  }
}

function requestLater(state: Runtime, target: GeometryTarget): void {
  const { body, shapeId, revision, bucket, key } = target;
  const request = requestFor(body, bucket);
  const promise = state.provider.request(request).catch(async (error: unknown) => {
    if (error instanceof OGError && error.code === 'LimitExceeded' && !state.failedBuckets.has(key)) {
      state.failedBuckets.add(key);
      state.buckets.set(shapeId, bucket * 2);
      emit('warning', { code: 'LimitExceeded', shapeId, revision, bucket, retryBucket: bucket * 2 });
      return state.provider.request(requestFor(body, bucket * 2));
    }
    throw error;
  }).then((buffers) => {
    const current = state.generations.get(shapeId) ?? 0;
    if (request.generation === current || buffers.bucket === bucket * 2) {
      receive(shapeId, revision, buffers.bucket, buffers);
    }
  }).catch((error: unknown) => {
    if (!(error instanceof OGError && error.code === 'Cancelled')) emit('error', error);
  }).finally(() => { state.pending.delete(key); });
  state.pending.set(key, promise);
}

function requestFor(body: Body, bucket: number): TessellationRequest {
  const state = runtime();
  const info = body.lastInfo;
  const shapeId = info.shapeId!;
  const generation = (state.generations.get(shapeId) ?? 0) + 1;
  state.generations.set(shapeId, generation);
  return { shapeId, revision: info.shapeRevision!, bucket, priority: 0, maxTriangles: 2_000_000, generation };
}

function receive(shapeId: string, revision: number, bucket: number, buffers: DisplayBuffers): void {
  const state = runtime();
  const showsShape = (body: Body): boolean =>
    body.lastInfo.shapeId === shapeId && body.lastInfo.shapeRevision === revision;
  if (![...state.bodies.values()].some(showsShape)) return;
  const record = state.records.put(new GeometryRecord(shapeId, buffers));
  state.readyVersion++;
  if (state.renderPassActive) {
    for (const body of state.bodies.values()) if (showsShape(body)) body.install(record);
  }
  const ogIds = [...state.bodies.values()].filter((body) => body.lastInfo.shapeId === shapeId).map((body) => body.ogId);
  emit('geometry', { shapeId, revision, bucket, ogIds, stats: { triangles: record.triangles } });
  if (!state.renderPassActive) flush();
}
