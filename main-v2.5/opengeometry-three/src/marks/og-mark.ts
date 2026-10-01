import type { MarkStats } from '../dto/mark-stats.js';
import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { flush } from '../rendering/geometry/geometry-scheduler.js';
import { bodyKey, runtime } from '../runtime/runtime-state.js';
import { decodeChanges, decodeMarkStats } from '../world-graph/codec.js';
import { worldGraph } from '../world-graph/world-graph-client.js';
import { finalizeNow, finalizeSettled, revive } from './limbo.js';

export class OGMark {
  constructor(private slot: number) {}
  private get active(): boolean { return runtime().marks.get(this.slot) === this; }
  rollback(): void {
    if (!this.active) throw new OGError('InvalidMark', 'OGMark.rollback', 'mark is released');
    const changes = decodeChanges(call('OGMark.rollback', () => worldGraph().rollback(this.slot)), 'OGMark.rollback');
    const state = runtime();
    for (const slot of [...state.marks.keys()]) if (slot > this.slot) state.marks.delete(slot);
    for (const change of changes.removed) {
      const body = state.bodies.get(bodyKey(change.handle, change.generation));
      if (body) finalizeNow(body);
    }
    flush();
    for (const change of changes.added) {
      const body = state.bodies.get(bodyKey(change.handle, change.generation));
      if (body) revive(body);
    }
  }
  release(): void {
    if (!this.active) throw new OGError('InvalidMark', 'OGMark.release', 'mark is released');
    call('OGMark.release', () => { worldGraph().release(this.slot); });
    const state = runtime();
    state.marks.delete(this.slot);
    finalizeSettled(state);
  }
}

export function createMark(): OGMark {
  const slot = call('OpenGeometry.mark', () => worldGraph().mark());
  const mark = new OGMark(slot);
  runtime().marks.set(slot, mark);
  return mark;
}

export function markStats(): MarkStats {
  return decodeMarkStats(call('OpenGeometry.markStats', () => worldGraph().markStats()), 'OpenGeometry.markStats');
}
