import { OGError } from '../../errors.js';

export function deflectionBucket(target: number): number {
  if (!Number.isFinite(target) || target <= 0) {
    throw new OGError('InvalidParameter', 'deflection', 'invalid display deflection');
  }
  return 2 ** Math.floor(Math.log2(target));
}

export function wantedBucket(target: number, floor: number, previous?: number, moving = false): number {
  const value = Math.max(floor, deflectionBucket(target));
  if (previous === undefined) return value;
  const kept = moving ? target >= 0.75 * previous : target >= 0.75 * previous && target <= 4 * previous;
  if (kept) return previous;
  return value;
}
