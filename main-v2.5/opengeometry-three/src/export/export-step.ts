import type { Body } from '../bodies/body.js';
import type { SystemAssembly } from '../bodies/system-assembly.js';
import { call } from '../kernel/kernel-session.js';
import { encode } from '../world-graph/codec.js';
import { worldGraph } from '../world-graph/world-graph-client.js';

export async function exportStep(options: {
  nodes: (Body | SystemAssembly | string)[];
  unit?: 'metre' | 'millimetre';
  upAxis?: 'Y' | 'Z';
  name?: string;
  timestamp?: string;
}) {
  const { nodes, ...other } = options;
  const value = call('OpenGeometry.exportStep', () => worldGraph().exportStep(
    encode(nodes.map((item) => typeof item === 'string' ? item : item.ogId)),
    encode(other),
  )) as { text: string; reportJson: string };
  return { text: value.text, report: JSON.parse(value.reportJson) };
}
