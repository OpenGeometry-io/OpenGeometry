import type { Body } from '../bodies/body';
import { SystemAssembly } from '../bodies/system-assembly';
import type { StepExport } from '../dto/step-export';
import { OGError } from '../errors';
import { call } from '../kernel/kernel-session';
import { bodyKey, runtime } from '../runtime/runtime-state';
import { decodeStepExport, encode } from '../world-graph/codec';
import { checkHandle, worldGraph } from '../world-graph/world-graph-client';

const LABEL = 'OpenGeometry.exportStep';

export function exportStep(options: {
  nodes: (Body | SystemAssembly | string)[];
  unit?: 'metre' | 'millimetre';
  upAxis?: 'Y' | 'Z';
  name?: string;
  timestamp?: string;
}): Promise<StepExport> {
  const { nodes, ...other } = options;
  return new Promise((resolve) => {
    for (const item of nodes) checkLive(item);
    const value = call(LABEL, (): unknown => worldGraph().exportStep(
      encode(nodes.map((item) => typeof item === 'string' ? item : item.ogId)),
      encode(other),
    ));
    resolve(decodeStepExport(value, LABEL));
  });
}

function checkLive(item: Body | SystemAssembly | string): void {
  if (typeof item === 'string') return;
  if (!(item instanceof SystemAssembly) && runtime().bodies.get(bodyKey(item.handle, item.generation)) !== item) {
    throw new OGError('Disposed', LABEL, `${item.ogId} is not a live body`);
  }
  checkHandle(LABEL, item);
}
