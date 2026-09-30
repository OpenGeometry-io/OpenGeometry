import type { Placement } from '../dto/placement.js';
import { call } from '../kernel/kernel-session.js';
import { flush } from '../rendering/geometry/geometry-scheduler.js';
import { encode } from '../world-graph/codec.js';
import { creation, node, worldGraph } from '../world-graph/world-graph-client.js';
import type { Body } from './body.js';

export class SystemAssembly {
  readonly ogId: string;
  readonly handle: number;
  readonly generation: number;

  constructor(options: { ogId?: string; parent?: SystemAssembly } = {}) {
    this.ogId = creation(call('SystemAssembly.constructor', () => worldGraph().createSystemAssembly(
      encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )));
    const info = node(this.ogId);
    this.handle = info.handle;
    this.generation = info.generation;
  }

  protected check(): void { call('SystemAssembly', () => worldGraph().nodeByHandle(this.handle, this.generation)); }

  transform(kind: string, params: Record<string, unknown>): void {
    this.check();
    call('SystemAssembly.transform', () => worldGraph().transform(this.ogId, encode({ kind, ...params })));
  }

  getPlacement(): Placement {
    this.check();
    return JSON.parse(call('SystemAssembly.getPlacement', () => worldGraph().placement(this.ogId)));
  }

  getWorldPlacement(): Placement {
    this.check();
    return JSON.parse(call('SystemAssembly.getWorldPlacement', () => worldGraph().worldPlacement(this.ogId)));
  }

  addChild(children: (Body | SystemAssembly)[], options: { keepWorld?: boolean } = {}): void {
    this.check();
    call('SystemAssembly.addChild', () => worldGraph().addChild(
      this.ogId, encode(children.map((child) => child.ogId)), Boolean(options.keepWorld),
    ));
  }

  removeChild(child: Body | SystemAssembly, options: { keepWorld?: boolean } = {}): void {
    this.check();
    call('SystemAssembly.removeChild', () => worldGraph().removeChild(
      this.ogId, child.ogId, Boolean(options.keepWorld),
    ));
  }

  getChildren(): string[] {
    this.check();
    return JSON.parse(call('SystemAssembly.getChildren', () => worldGraph().children(this.ogId)));
  }

  getParent(): string | null {
    this.check();
    return JSON.parse(call('SystemAssembly.getParent', () => worldGraph().parent(this.ogId)));
  }

  getBounds(): [number, number, number, number, number, number] | null {
    this.check();
    return JSON.parse(call('SystemAssembly.getBounds', () => worldGraph().bounds(this.ogId)));
  }

  dispose(): void {
    this.check();
    call('SystemAssembly.dispose', () => worldGraph().dispose(this.ogId));
    flush();
  }
}
