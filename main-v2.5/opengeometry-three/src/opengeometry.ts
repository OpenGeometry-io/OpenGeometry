import type * as THREE from 'three';
import type { Body } from './bodies/body.js';
import type { SystemAssembly } from './bodies/system-assembly.js';
import type { Events, Listener } from './dto/events.js';
import type { MarkStats } from './dto/mark-stats.js';
import type { StepExport } from './dto/step-export.js';
import { OGError } from './errors.js';
import { exportStep } from './export/export-step.js';
import { createMark, markStats, type OGMark } from './marks/og-mark.js';
import { transaction } from './marks/transaction.js';
import { resolveHit } from './picking/resolve-hit.js';
import { flush } from './rendering/geometry/geometry-scheduler.js';
import { settled } from './rendering/geometry/settle.js';
import { createRuntimeOnce } from './runtime/create-runtime.js';
import { on } from './runtime/event-bus.js';
import { reset } from './runtime/reset.js';
import { currentRuntime, runtime } from './runtime/runtime-state.js';

export class OpenGeometry {
  static async create(
    input: { wasmURL?: string | URL; wasmModule?: WebAssembly.Module } = {},
    options: { workerURL?: string | URL; tessellation?: 'worker' | 'inline' } = {},
  ): Promise<typeof OpenGeometry> {
    if (currentRuntime()) return this;
    await createRuntimeOnce(input, options);
    return this;
  }

  static on(event: Events, handler: Listener): () => boolean { return on(event, handler); }
  static flush(options: { geometry?: 'sync' } = {}): void { flush(options); }
  static settled(options: { deflection?: number } = {}): Promise<{ failed: { ogId: string; error: unknown }[] }> {
    return settled(options);
  }

  static setDisplayDeflection(world?: number): void {
    if (world !== undefined && (!Number.isFinite(world) || world <= 0)) {
      throw new OGError('InvalidParameter', 'setDisplayDeflection', 'deflection must be positive');
    }
    Object.assign(runtime(), { displayDeflection: world });
    runtime().buckets.clear();
    runtime().failedBuckets.clear();
  }

  static setCameraMotion(moving: boolean): void { runtime().moving = moving; }

  static resolveHit(
    hit: THREE.Intersection,
  ): { ogId: string; shapeRevision: number; faceId?: number; edgeId?: number } | undefined {
    return resolveHit(hit);
  }

  static exportStep(options: {
    nodes: (Body | SystemAssembly | string)[];
    unit?: 'metre' | 'millimetre';
    upAxis?: 'Y' | 'Z';
    name?: string;
    timestamp?: string;
  }): Promise<StepExport> { return exportStep(options); }

  static mark(): OGMark { return createMark(); }
  static markStats(): MarkStats { return markStats(); }
  static transaction<T>(fn: () => T, options: { dryRun?: boolean } = {}): T { return transaction(fn, options); }

  static reset(): void { reset(); }
}
