import {
  Wire, Solid, OG_PRIMITIVE_RECTANGLE, OG_PRIMITIVE_CUBOID, OG_OPERATION_EXTRUDE,
} from '../../dist/index.js';
import { graph } from '../../dist/testing.js';

export function mesh(body) {
  const info = JSON.parse(graph().node(body.ogId));
  return graph().buffers(info.shapeId, 0.01, 2_000_000);
}

export function cutters(count, prefix, width, step, rotation = 0) {
  const tools = [];
  for (let i = 0; i < count; i++) {
    const tool = new Solid(OG_PRIMITIVE_CUBOID, { width: 0.5, height: 2, depth: 0.4 }, { ogId: `${prefix}-cut-${i}` });
    tool.transform('Translate', { offset: [-width / 2 + 3 + i * step, 0, 0] });
    if (rotation) tool.transform('Rotate', { axis: [0, 1, 0], degrees: rotation, pivot: [0, 0, 0] });
    tools.push(tool);
  }
  return tools;
}

export function wall(prefix, width, height = 3) {
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width, breadth: 0.2 }, { ogId: `${prefix}-profile` });
  return { profile, body: new Solid(OG_OPERATION_EXTRUDE, { profile, distance: height }, { ogId: `${prefix}-wall` }) };
}
