import type * as THREE from 'three';
import type { Body } from './bodies/body.js';
import { register, unregister } from './bodies/body-registry.js';
import type { SystemAssembly } from './bodies/system-assembly.js';
import type { Events, Listener } from './dto/events.js';
import { OGError } from './errors.js';
import { exportStep } from './export/export-step.js';
import { compileKernel, initKernel, type OGWorldGraph } from './kernel/kernel-loader.js';
import { createMark, markStats, type OGMark } from './marks/og-mark.js';
import { transaction } from './marks/transaction.js';
import { resolveHit } from './picking/resolve-hit.js';
import { ensureGeometry, flush, wanted } from './rendering/geometry/geometry-scheduler.js';
import { noteDisplayed } from './rendering/geometry/render-pass.js';
import { settled } from './rendering/geometry/settle.js';
import { createRuntime } from './runtime/create-runtime.js';
import { emit, on } from './runtime/event-bus.js';
import { reset } from './runtime/reset.js';
import { currentRuntime, runtime, setRuntime, type Runtime } from './runtime/runtime-state.js';
import { worldGraph } from './world-graph/world-graph-client.js';

export class OpenGeometry {
  static async create(
    input: { wasmURL?: string | URL; wasmModule?: WebAssembly.Module } = {},
    options: { workerURL?: string | URL; tessellation?: 'worker' | 'inline' } = {},
  ): Promise<typeof OpenGeometry> {
    if (currentRuntime()) return this;
    const wasmURL = input.wasmURL ?? new URL('./opengeometry_bg.wasm', import.meta.url);
    const module = input.wasmModule ?? await compileKernel(wasmURL);
    await initKernel(module);
    setRuntime(createRuntime(module, options));
    return this;
  }

  static runtime(): Runtime { return runtime(); }
  static graph(): OGWorldGraph { return worldGraph(); }
  static get activeBackend(): "worker" | "inline" { return this.runtime().provider.activeBackend; }
  static get flushCount(): number { return this.runtime().flushes; }
  static on(event: Events, handler: Listener): () => boolean { return on(event, handler); }
  static emit(event: Events, detail: unknown): void { emit(event, detail); }

  static register(body: Body): void { register(body); }
  static unregister(body: Body): void { unregister(body); }
  static noteDisplayed(body: Body): void { noteDisplayed(body); }
  static flush(options: { geometry?: 'sync' } = {}): void { flush(options); }
  static wanted(body: Body): number { return wanted(body); }
  static ensureGeometry(body: Body, sync = false): void { ensureGeometry(body, sync); }
  static settled(options: { deflection?: number } = {}): Promise<{ failed: { ogId: string; error: unknown }[] }> {
    return settled(options);
  }

  static setDisplayDeflection(world?: number): void {
    if (world !== undefined && (!Number.isFinite(world) || world <= 0)) {
      throw new OGError('InvalidParameter', 'setDisplayDeflection', 'deflection must be positive');
    }
    Object.assign(this.runtime(), { displayDeflection: world });
    this.runtime().buckets.clear();
    this.runtime().failedBuckets.clear();
  }

  static setCameraMotion(moving: boolean): void { this.runtime().moving = moving; }

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
  }) { return exportStep(options); }

  static mark(): OGMark { return createMark(); }
  static markStats() { return markStats(); }
  static transaction<T>(fn: () => T, options: { dryRun?: boolean } = {}): T { return transaction(fn, options); }

  static reset(): void { reset(); }
}
