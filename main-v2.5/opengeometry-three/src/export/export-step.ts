import type { Body } from '../bodies/body.js';
import type { SystemAssembly } from '../bodies/system-assembly.js';
import type { StepExport } from '../dto/step-export.js';
import { call } from '../kernel/kernel-session.js';
import { decodeStepExport, encode } from '../world-graph/codec.js';
import { worldGraph } from '../world-graph/world-graph-client.js';

export async function exportStep(options: {
  nodes: (Body | SystemAssembly | string)[];
  unit?: 'metre' | 'millimetre';
  upAxis?: 'Y' | 'Z';
  name?: string;
  timestamp?: string;
}): Promise<StepExport> {
  const { nodes, ...other } = options;
  const value = call('OpenGeometry.exportStep', (): unknown => worldGraph().exportStep(
    encode(nodes.map((item) => typeof item === 'string' ? item : item.ogId)),
    encode(other),
  ));
  return decodeStepExport(value, 'OpenGeometry.exportStep');
}
