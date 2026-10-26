import type * as THREE from 'three';
import type { Body } from './bodies/body';
import type { SystemAssembly } from './bodies/system-assembly';
import type { Events, Listener } from './dto/events';
import type { AnalyticExchangeBodyV2 } from './dto/ifc-body';
import type { MarkStats } from './dto/mark-stats';
import type { CurvedRegion2D, OffsetRegion, RegionOperation } from './dto/regions';
import type { StepExport } from './dto/step-export';
import { OGError } from './errors';
import { exportStep } from './export/export-step';
import { exportIfcBody } from './export/ifc-body';
import { createMark, markStats, type OGMark } from './marks/og-mark';
import { transaction } from './marks/transaction';
import { booleanCurvedRegions2D, booleanRegions2D } from './operations/regions';
import { resolveHit } from './picking/resolve-hit';
import { flush } from './rendering/geometry/geometry-scheduler';
import { settled } from './rendering/geometry/settle';
import { motionHint, overrideChanged } from './rendering/lod/lod-controller';
import { createRuntimeOnce } from './runtime/create-runtime';
import { on } from './runtime/event-bus';
import { reset } from './runtime/reset';
import { currentRuntime, runtime } from './runtime/runtime-state';

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
    overrideChanged(runtime());
  }

  static setCameraMotion(moving: boolean): void {
    const state = runtime();
    if (moving) motionHint(state, Date.now());
  }

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

  static exportIfcBody(node: Body | SystemAssembly): AnalyticExchangeBodyV2 { return exportIfcBody(node); }

  static booleanRegions2D(
    a: readonly OffsetRegion[],
    b: readonly OffsetRegion[],
    operation: RegionOperation,
  ): OffsetRegion[] { return booleanRegions2D(a, b, operation); }

  static booleanCurvedRegions2D(
    a: readonly CurvedRegion2D[],
    b: readonly CurvedRegion2D[],
    operation: RegionOperation,
  ): CurvedRegion2D[] { return booleanCurvedRegions2D(a, b, operation); }

  static mark(): OGMark { return createMark(); }
  static markStats(): MarkStats { return markStats(); }
  static transaction<T>(fn: () => T, options: { dryRun?: boolean } = {}): T { return transaction(fn, options); }

  static reset(): void { reset(); }
}
