import { OGError } from '../../errors';
import { runtime, type Runtime } from '../../runtime/runtime-state';
import type { Body } from '../../bodies/body';
import { displayedBodies } from '../lod/displayed';
import {
  capturedError, popTemporaryTarget, pushTemporaryTarget, settleNow, wantedKey,
} from '../lod/lod-controller';
import { flush } from './geometry-scheduler';
import { refreshDisplayed } from './render-pass';

type Failure = { ogId: string; error: unknown };

export async function settled(options: { deflection?: number } = {}): Promise<{ failed: Failure[] }> {
  const state = runtime();
  const token = options.deflection === undefined ? undefined : pushTemporaryTarget(state, options.deflection);
  try {
    settleNow(state);
    refreshDisplayed(state);
    const wanted = new Map(displayedBodies(state).map((body) => [body, wantedKey(state, body)]));
    do await Promise.all([...state.pending.values()]);
    while (state.pending.size > 0 && unresolved(state, wanted));
    flush();
    return { failed: failures(state) };
  } finally {
    if (token) popTemporaryTarget(state, token);
  }
}

function unresolved(state: Runtime, wanted: ReadonlyMap<Body, number | undefined>): boolean {
  return [...wanted].some(([body, key]) => !currentAt(body, key) && capturedError(state, body) === undefined);
}

function failures(state: Runtime): Failure[] {
  const failed: Failure[] = [];
  for (const body of displayedBodies(state)) {
    if (current(state, body)) continue;
    const error = capturedError(state, body) ?? new OGError('Cancelled', 'OpenGeometry.settled', 'no current record');
    failed.push({ ogId: body.ogId, error });
  }
  return failed;
}

function current(state: Runtime, body: Body): boolean {
  return currentAt(body, wantedKey(state, body));
}

function currentAt(body: Body, key: number | undefined): boolean {
  const { record, lastInfo } = body;
  const fresh = record?.shapeId === lastInfo.shapeId && record.revision === lastInfo.shapeRevision;
  return fresh && record.bucket === key;
}
