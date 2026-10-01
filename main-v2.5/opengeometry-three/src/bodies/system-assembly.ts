import type { Placement } from '../dto/placement.js';
import { call } from '../kernel/kernel-session.js';
import { flush } from '../rendering/geometry/geometry-scheduler.js';
import { runtime } from '../runtime/runtime-state.js';
import { encode } from '../world-graph/codec.js';
import { creation, node, worldGraph } from '../world-graph/world-graph-client.js';
import type { Body } from './body.js';
import {
  addChild, checkNode, getBounds, getChildren, getParent, getPlacement, getWorldPlacement, removeChild,
} from './node-methods.js';

export class SystemAssembly {
  readonly ogId: string;
  readonly handle: number;
  readonly generation: number;
  private readonly epoch = runtime().epoch;

  constructor(options: { ogId?: string; parent?: SystemAssembly } = {}) {
    this.ogId = creation(call('SystemAssembly.constructor', () => worldGraph().createSystemAssembly(
      encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )));
    const info = node(this.ogId);
    this.handle = info.handle;
    this.generation = info.generation;
  }

  protected check(): void { checkNode('SystemAssembly', 'assembly', this.epoch, this.handle, this.generation); }

  transform(kind: string, params: Record<string, unknown>): void {
    this.check();
    call('SystemAssembly.transform', () => worldGraph().transform(this.ogId, encode({ kind, ...params })));
  }

  getPlacement(): Placement { this.check(); return getPlacement('SystemAssembly', this.ogId); }

  getWorldPlacement(): Placement { this.check(); return getWorldPlacement('SystemAssembly', this.ogId); }

  addChild(children: (Body | SystemAssembly)[], options: { keepWorld?: boolean } = {}): void {
    this.check();
    addChild('SystemAssembly', this.ogId, children, Boolean(options.keepWorld));
  }

  removeChild(child: Body | SystemAssembly, options: { keepWorld?: boolean } = {}): void {
    this.check();
    removeChild('SystemAssembly', this.ogId, child.ogId, Boolean(options.keepWorld));
  }

  getChildren(): string[] { this.check(); return getChildren('SystemAssembly', this.ogId); }

  getParent(): string | null { this.check(); return getParent('SystemAssembly', this.ogId); }

  getBounds(): [number, number, number, number, number, number] | null {
    this.check();
    return getBounds('SystemAssembly', this.ogId);
  }

  dispose(): void {
    this.check();
    call('SystemAssembly.dispose', () => worldGraph().dispose(this.ogId));
    flush();
  }
}
