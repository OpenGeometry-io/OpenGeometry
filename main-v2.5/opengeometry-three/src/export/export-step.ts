import type { Body } from '../bodies/body.js';
import { SystemAssembly } from '../bodies/system-assembly.js';
import type { StepExport } from '../dto/step-export.js';
import { OGError } from '../errors.js';
import { call } from '../kernel/kernel-session.js';
import { bodyKey, runtime } from '../runtime/runtime-state.js';
import { decodeStepExport, encode } from '../world-graph/codec.js';
import { checkHandle, worldGraph } from '../world-graph/world-graph-client.js';

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
