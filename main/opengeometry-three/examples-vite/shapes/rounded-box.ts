import { startParametricSolid } from '../shared/parametric-solid';
import { OG_PRIMITIVE_ROUNDED_BOX, OG_PRIMITIVE_PARAMS_ROUNDED_BOX } from '../../../dist/index.js';

await startParametricSolid({
  kind: OG_PRIMITIVE_ROUNDED_BOX,
  title: 'Rounded box',
  params: OG_PRIMITIVE_PARAMS_ROUNDED_BOX({ width: 1.2, height: 0.6, depth: 0.8, radius: 0.15 }),
});
