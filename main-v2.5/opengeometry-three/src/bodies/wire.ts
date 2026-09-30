import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { encode } from '../world-graph/codec.js';
import { creation, worldGraph } from '../world-graph/world-graph-client.js';
import { Body } from './body.js';
import { createBody } from './body-factory.js';
import type { BodyOptions } from './body-options.js';
import type { SystemAssembly } from './system-assembly.js';

export class Wire extends Body {
  constructor(kind: string, params: Record<string, unknown>, options: BodyOptions = {}) {
    super(createBody(kind, params, options, 'Wire'), 'Wire', options);
    if (this.lastInfo.bodyType !== 'Wire') throw new OGError('BodyTypeMismatch', 'Wire.constructor', 'expected Wire');
  }
  instance(options: { ogId?: string; parent?: SystemAssembly } = {}): Wire {
    this.check();
    const ogId = creation(call('Wire.instance', () => worldGraph().instance(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )));
    return new Wire('__existing', {}, { ogId });
  }
  duplicate(options: { ogId?: string; parent?: SystemAssembly } = {}): Wire {
    this.check();
    const ogId = creation(call('Wire.duplicate', () => worldGraph().duplicate(
      this.ogId, encode({ ogId: options.ogId, parent: options.parent?.ogId }),
    )));
    return new Wire('__existing', {}, { ogId });
  }
}
