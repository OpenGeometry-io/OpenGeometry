import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { flush } from '../rendering/geometry/geometry-scheduler.js';
import { runtime } from '../runtime/runtime-state.js';
import { worldGraph } from '../world-graph/world-graph-client.js';

export class OGMark {
  private active = true;
  constructor(private slot: number) {}
  rollback(): void {
    if (!this.active) throw new OGError('InvalidMark', 'OGMark.rollback', 'mark is released');
    call('OGMark.rollback', () => worldGraph().rollback(this.slot));
    flush();
    for (const body of runtime().bodies.values()) body.reviveIfRestored();
  }
  release(): void {
    if (!this.active) throw new OGError('InvalidMark', 'OGMark.release', 'mark is released');
    call('OGMark.release', () => { worldGraph().release(this.slot); });
    this.active = false;
    runtime().marks.delete(this);
    if (runtime().marks.size === 0) {
      for (const body of [...runtime().bodies.values()]) if (body.inLimbo) body.finalize();
    }
  }
}

export function createMark(): OGMark {
  const mark = new OGMark(call('OpenGeometry.mark', () => worldGraph().mark()));
  runtime().marks.add(mark);
  return mark;
}

export function markStats() { return JSON.parse(call('OpenGeometry.markStats', () => worldGraph().markStats())); }
