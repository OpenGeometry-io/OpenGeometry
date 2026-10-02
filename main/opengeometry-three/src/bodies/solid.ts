import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { flush } from '../rendering/geometry/geometry-scheduler';
import type { ShapeReport } from '../dto/boolean-report';
import { decodeShapeReport, encode, scope } from '../world-graph/codec';
import { creation, worldGraph } from '../world-graph/world-graph-client';
import { Body } from './body';
import { createBody, existingParams } from './body-factory';
import type { BodyOptions } from './body-options';
import type { SystemAssembly } from './system-assembly';

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
