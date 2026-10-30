import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { encode } from '../world-graph/codec';
import { creation, worldGraph } from '../world-graph/world-graph-client';
import { Body } from './body';
import { createBody, existingParams } from './body-factory';
import type { BodyOptions } from './body-options';
import type { SystemAssembly } from './system-assembly';

export class Sheet extends Body {
  constructor(kind: string, params: Record<string, unknown>, options: BodyOptions = {}) {
    super(createBody(kind, params, options, 'Sheet'), 'Sheet', options);
    if (this.lastInfo.bodyType !== 'Sheet') {
      throw new OGError('BodyTypeMismatch', 'Sheet.constructor', 'expected Sheet');
    }
  }
  instance(options: { ogId?: string; parent?: SystemAssembly } = {}): Sheet {
    this.check('Sheet.instance');
    const ogId = creation(call('Sheet.instance', () => worldGraph().instance(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'Sheet.instance');
    return new Sheet('', existingParams(ogId));
  }
  duplicate(options: { ogId?: string; parent?: SystemAssembly } = {}): Sheet {
    this.check('Sheet.duplicate');
    const ogId = creation(call('Sheet.duplicate', () => worldGraph().duplicate(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'Sheet.duplicate');
    return new Sheet('', existingParams(ogId));
  }
}
