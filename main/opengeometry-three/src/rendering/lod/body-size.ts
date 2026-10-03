import type { Body } from '../../bodies/body';
import { OGError } from '../../errors';
import type { Runtime } from '../../runtime/runtime-state';
import { displayBuckets } from '../../world-graph/world-graph-client';

export type ShapeBuckets = { revision: number; floor: number; static: number; error?: unknown };

export function shapeBuckets(state: Runtime, body: Body): ShapeBuckets | undefined {
  const { shapeId, shapeRevision } = body.lastInfo;
  if (!shapeId || shapeRevision === null) return undefined;
  let entry = state.shapeBuckets.get(shapeId);
  if (entry?.revision !== shapeRevision) {
    entry = readBuckets(body.ogId, shapeRevision);
    state.shapeBuckets.set(shapeId, entry);
  }
  return entry.error === undefined ? entry : undefined;
}

function readBuckets(ogId: string, revision: number): ShapeBuckets {
  try {
    return { revision, ...displayBuckets(ogId, 'OpenGeometry.flush') };
  } catch (error) {
    if (!(error instanceof OGError) || error.code === 'KernelPanic') throw error;
    return { revision, floor: 0, static: 0, error };
  }
}

export function localTarget(world: number, body: Body): number | undefined {
  const scale = worldScale(body);
  return scale > 0 && Number.isFinite(scale) ? world / scale : undefined;
}

function worldScale(body: Body): number {
  return Math.cbrt(Math.abs(body.matrixWorld.determinant()));
}

export function finestLocal(bodies: Iterable<Body>, valueOf: (body: Body) => number | undefined): number | undefined {
  let finest: number | undefined;
  for (const body of bodies) {
    const value = valueOf(body);
    const target = value === undefined ? undefined : localTarget(value, body);
    if (target !== undefined) finest = finest === undefined ? target : Math.min(finest, target);
  }
  return finest;
}
