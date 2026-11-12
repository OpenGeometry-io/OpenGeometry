import { startParametricSolid } from '../shared/parametric-solid';
import { OG_PRIMITIVE_SLOT_PRISM, OG_PRIMITIVE_PARAMS_SLOT_PRISM } from '../../../dist/index.js';

await startParametricSolid({
  kind: OG_PRIMITIVE_SLOT_PRISM,
  title: 'Slot prism',
  params: OG_PRIMITIVE_PARAMS_SLOT_PRISM({ length: 1, width: 0.5, height: 0.4 }),
});
