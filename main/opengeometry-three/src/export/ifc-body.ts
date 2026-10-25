import type { Body } from '../bodies/body';
import { SystemAssembly } from '../bodies/system-assembly';
import type { AnalyticExchangeBodyV2 } from '../dto/ifc-body';
import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { bodyKey, runtime } from '../runtime/runtime-state';
import { decodeIfcBody } from '../world-graph/codec';
import { checkHandle, worldGraph } from '../world-graph/world-graph-client';

const LABEL = 'OpenGeometry.exportIfcBody';

export function exportIfcBody(node: Body | SystemAssembly): AnalyticExchangeBodyV2 {
  if (!(node instanceof SystemAssembly) && runtime().bodies.get(bodyKey(node.handle, node.generation)) !== node) {
    throw new OGError('Disposed', LABEL, `${node.ogId} is not a live body`);
  }
  checkHandle(LABEL, node);
  return decodeIfcBody(call(LABEL, () => worldGraph().exportIfcBody(node.ogId)), LABEL);
}
