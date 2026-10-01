import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { encode } from '../world-graph/codec';
import { creation, worldGraph } from '../world-graph/world-graph-client';
import { Body } from './body';
import { createBody, existingParams } from './body-factory';
import type { BodyOptions } from './body-options';
import type { SystemAssembly } from './system-assembly';

export class Wire extends Body {
  constructor(kind: string, params: Record<string, unknown>, options: BodyOptions = {}) {
    super(createBody(kind, params, options, 'Wire'), 'Wire', options);
    if (this.lastInfo.bodyType !== 'Wire') throw new OGError('BodyTypeMismatch', 'Wire.constructor', 'expected Wire');
  }
  instance(options: { ogId?: string; parent?: SystemAssembly } = {}): Wire {
    this.check('Wire.instance');
    const ogId = creation(call('Wire.instance', () => worldGraph().instance(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'Wire.instance');
    return new Wire('', existingParams(ogId));
  }
  duplicate(options: { ogId?: string; parent?: SystemAssembly } = {}): Wire {
    this.check('Wire.duplicate');
    const ogId = creation(call('Wire.duplicate', () => worldGraph().duplicate(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )), 'Wire.duplicate');
    return new Wire('', existingParams(ogId));
  }
}
