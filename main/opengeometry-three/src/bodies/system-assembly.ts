import type { Placement } from '../dto/placement';
import { call } from '../kernel/kernel-session';
import { flush } from '../rendering/geometry/geometry-scheduler';
import { runtime } from '../runtime/runtime-state';
import { encode } from '../world-graph/codec';
import { creation, node, worldGraph } from '../world-graph/world-graph-client';
import type { Body } from './body';
import {
  addChild, checkNode, getBounds, getChildren, getParent, getPlacement, getWorldPlacement, removeChild,
} from './node-methods';

export class SystemAssembly {
  readonly ogId: string;
  readonly handle: number;
  readonly generation: number;
  private readonly epoch = runtime().epoch;

  constructor(options: { ogId?: string; parent?: SystemAssembly } = {}) {
    this.ogId = creation(call('SystemAssembly.constructor', () => worldGraph().createSystemAssembly(
      encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'SystemAssembly.constructor');
    const info = node(this.ogId, 'SystemAssembly.constructor');
    this.handle = info.handle;
    this.generation = info.generation;
  }

  protected check(call: string): void { checkNode(call, 'assembly', this.epoch, this.handle, this.generation); }

  transform(kind: string, params: Record<string, unknown>): void {
    this.check('SystemAssembly.transform');
    call('SystemAssembly.transform', () => worldGraph().transform(this.ogId, encode({ kind, ...params })));
  }

  getPlacement(): Placement {
    this.check('SystemAssembly.getPlacement');
    return getPlacement('SystemAssembly', this.ogId);
  }

  getWorldPlacement(): Placement {
    this.check('SystemAssembly.getWorldPlacement');
    return getWorldPlacement('SystemAssembly', this.ogId);
  }

  addChild(children: (Body | SystemAssembly)[], options: { keepWorld?: boolean } = {}): void {
    this.check('SystemAssembly.addChild');
    addChild('SystemAssembly', this.ogId, children, Boolean(options.keepWorld));
  }

  removeChild(child: Body | SystemAssembly, options: { keepWorld?: boolean } = {}): void {
    this.check('SystemAssembly.removeChild');
    removeChild('SystemAssembly', this.ogId, child.ogId, Boolean(options.keepWorld));
  }

  getChildren(): string[] { this.check('SystemAssembly.getChildren'); return getChildren('SystemAssembly', this.ogId); }

  getParent(): string | null { this.check('SystemAssembly.getParent'); return getParent('SystemAssembly', this.ogId); }

  getBounds(): [number, number, number, number, number, number] | null {
    this.check('SystemAssembly.getBounds');
    return getBounds('SystemAssembly', this.ogId);
  }

  dispose(): void {
    this.check('SystemAssembly.dispose');
    call('SystemAssembly.dispose', () => worldGraph().dispose(this.ogId));
    flush();
  }
}
