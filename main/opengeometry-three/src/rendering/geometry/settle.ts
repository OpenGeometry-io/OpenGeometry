import { OGError } from '../../errors';
import { runtime, type Runtime } from '../../runtime/runtime-state';
import {
  capturedError, displayedBodies, popTemporaryTarget, pushTemporaryTarget, settleNow, wantedKey,
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
    await Promise.all([...state.pending.values()]);
    flush();
    return { failed: failures(state) };
  } finally {
    if (token) popTemporaryTarget(state, token);
  }
}

function failures(state: Runtime): Failure[] {
  const failed: Failure[] = [];
  for (const body of displayedBodies(state)) {
    const { record, lastInfo } = body;
    const current = record?.shapeId === lastInfo.shapeId && record.revision === lastInfo.shapeRevision;
    if (current && record.bucket === wantedKey(state, body)) continue;
    const error = capturedError(state, body) ?? new OGError('Cancelled', 'OpenGeometry.settled', 'no current record');
    failed.push({ ogId: body.ogId, error });
  }
  return failed;
}
