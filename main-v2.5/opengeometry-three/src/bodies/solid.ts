import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { flush } from '../rendering/geometry/geometry-scheduler.js';
import type { ShapeReport } from '../dto/boolean-report.js';
import { decodeShapeReport, encode, scope } from '../world-graph/codec.js';
import { creation, worldGraph } from '../world-graph/world-graph-client.js';
import { Body } from './body.js';
import { createBody, existingParams } from './body-factory.js';
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
    this.check('Solid.operate');
    call('Solid.operate', () => worldGraph().operate(
      this.ogId, encode(kind), encode(options.tools.map((tool) => tool.ogId)), scope(options),
    ));
    flush();
  }
  getReport(): ShapeReport | null {
    this.check('Solid.getReport');
    return decodeShapeReport(call('Solid.getReport', () => worldGraph().report(this.ogId)), 'Solid.getReport');
  }
  instance(options: { ogId?: string; parent?: SystemAssembly } = {}): Solid {
    this.check('Solid.instance');
    const ogId = creation(call('Solid.instance', () => worldGraph().instance(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'Solid.instance');
    return new Solid('', existingParams(ogId));
  }
  duplicate(options: { ogId?: string; parent?: SystemAssembly } = {}): Solid {
    this.check('Solid.duplicate');
    const ogId = creation(call('Solid.duplicate', () => worldGraph().duplicate(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'Solid.duplicate');
    return new Solid('', existingParams(ogId));
  }
}
