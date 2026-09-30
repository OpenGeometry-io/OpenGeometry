import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { flush } from '../rendering/geometry/geometry-scheduler.js';
import { encode, scope } from '../world-graph/codec.js';
import { creation, worldGraph } from '../world-graph/world-graph-client.js';
import { Body } from './body.js';
import { createBody } from './body-factory.js';
import type { BodyOptions } from './body-options.js';
import type { SystemAssembly } from './system-assembly.js';

export class Solid extends Body {
  constructor(kind: string, params: Record<string, unknown>, options: BodyOptions = {}) {
    super(createBody(kind, params, options, 'Solid'), 'Solid', options);
    if (this.lastInfo.bodyType !== 'Solid') {
      throw new OGError('BodyTypeMismatch', 'Solid.constructor', 'expected Solid');
    }
  }
  operate(kind: string, options: { tools: Solid[]; instances?: 'all' }): void {
    this.check();
    call('Solid.operate', () => worldGraph().operate(
      this.ogId, encode(kind), encode(options.tools.map((tool) => tool.ogId)), scope(options),
    ));
    flush();
  }
  getReport() { this.check(); return JSON.parse(call('Solid.getReport', () => worldGraph().report(this.ogId))); }
  instance(options: { ogId?: string; parent?: SystemAssembly } = {}): Solid {
    this.check();
    const ogId = creation(call('Solid.instance', () => worldGraph().instance(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )));
    return new Solid('__existing', {}, { ogId });
  }
  duplicate(options: { ogId?: string; parent?: SystemAssembly } = {}): Solid {
    this.check();
    const ogId = creation(call('Solid.duplicate', () => worldGraph().duplicate(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )));
    return new Solid('__existing', {}, { ogId });
  }
}
