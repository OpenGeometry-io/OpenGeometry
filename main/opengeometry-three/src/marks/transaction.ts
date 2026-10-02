import { OGError } from '../errors';
import { createMark } from './og-mark';

export function transaction<T>(fn: () => T, options: { dryRun?: boolean } = {}): T {
  const mark = createMark();
  try {
    const result = fn();
    if (result && typeof (result as { then?: unknown }).then === 'function') {
      throw new OGError('InvalidParameter', 'OpenGeometry.transaction', 'transaction callback must be synchronous');
    }
    if (options.dryRun) mark.rollback();
    mark.release();
    return result;
  } catch (error) {
    mark.rollback();
    mark.release();
    throw error;
  }
}
