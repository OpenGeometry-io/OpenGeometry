import type { TessellateMessage } from '../worker-protocol.js';

export class JobQueue {
  private readonly jobs = new Map<string, TessellateMessage>();

  get size(): number {
    return this.jobs.size;
  }

  push(job: TessellateMessage): number | undefined {
    const previous = this.jobs.get(job.shapeId);
    this.jobs.set(job.shapeId, job);
    return previous?.request;
  }

  cancel(shapeId: string, generation: number): number | undefined {
    const previous = this.jobs.get(shapeId);
    if (!previous || previous.generation > generation) return undefined;
    this.jobs.delete(shapeId);
    return previous.request;
  }

  next(): TessellateMessage | undefined {
    const next = [...this.jobs.values()].sort((a, b) => a.priority - b.priority)[0];
    if (next) this.jobs.delete(next.shapeId);
    return next;
  }
}
