import { startParametricSolid } from '../shared/parametric-solid';
import { OG_PRIMITIVE_PIPE_SEGMENT, OG_PRIMITIVE_PARAMS_PIPE_SEGMENT } from '../../../dist/index.js';

await startParametricSolid({
  kind: OG_PRIMITIVE_PIPE_SEGMENT,
  title: 'Pipe segment',
  params: OG_PRIMITIVE_PARAMS_PIPE_SEGMENT({
    outerRadius: 0.8,
    innerRadius: 0.6,
    height: 0.5,
    startAngle: 0,
    sweepAngle: 1.57,
  }),
});
