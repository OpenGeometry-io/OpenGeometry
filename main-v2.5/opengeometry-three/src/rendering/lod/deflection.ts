import { OGError } from '../../errors.js';
import { HYSTERESIS_KEEP } from '../../limits.js';

export function deflectionBucket(target: number): number {
  if (!Number.isFinite(target) || target <= 0) {
    throw new OGError('InvalidParameter', 'deflection', 'invalid display deflection');
  }
  return 2 ** Math.floor(Math.log2(target));
}

export function wantedBucket(target: number, floor: number, previous?: number, moving = false): number {
  const value = Math.max(floor, deflectionBucket(target));
  if (previous === undefined) return value;
  const [low, high] = HYSTERESIS_KEEP;
  const kept = moving ? target >= low * previous : target >= low * previous && target <= high * previous;
  if (kept) return previous;
  return value;
}
