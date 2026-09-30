import { runtime } from '../../runtime/runtime-state.js';
import { ensureGeometry, flush } from './geometry-scheduler.js';

export async function settled(
  options: { deflection?: number } = {},
): Promise<{ failed: { ogId: string; error: unknown }[] }> {
  const state = runtime();
  const failed: { ogId: string; error: unknown }[] = [];
  for (const body of state.displayed) {
    if (options.deflection !== undefined) body.setAppearance({ deflection: options.deflection });
    ensureGeometry(body);
  }
  await Promise.all([...state.pending.values()]);
  flush();
  for (const body of state.displayed) {
    if (!body.record || body.record.revision !== body.lastInfo.shapeRevision) {
      failed.push({ ogId: body.ogId, error: 'No current geometry record' });
    }
  }
  return { failed };
}
