import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
import { OpenGeometry, OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT } from '../../dist/index.js';
import { createInlineKernel } from '../lib/kernel.mjs';
import { cutters, mesh, wall } from './scenes.mjs';

await createInlineKernel();

try {
  const first = wall('straight', 60);
  const fifty = cutters(50, 'straight', 60, 1.1);
  let start = performance.now();
  first.body.operate(OG_OPERATION_SUBTRACT, { tools: fifty });
  const wall50BooleanMs = performance.now() - start;
  start = performance.now();
  const wall50Triangles = mesh(first.body).triangles;
  const wall50TessellationMs = performance.now() - start;

  const angled = wall('angled', 30);
  angled.body.transform('Rotate', { axis: [0, 1, 0], degrees: 30, pivot: [0, 0, 0] });
  const twenty = cutters(20, 'angled', 30, 1.35, 30);
  start = performance.now();
  angled.body.operate(OG_OPERATION_SUBTRACT, { tools: twenty });
  const rotated20BooleanMs = performance.now() - start;

  const slider = wall('slider', 6);
  const four = cutters(4, 'slider', 6, 1.2);
  start = performance.now();
  slider.body.rebuild(OG_OPERATION_EXTRUDE, { profile: slider.profile, distance: 4 });
  const sliderRebuildMs = performance.now() - start;
  start = performance.now();
  slider.body.operate(OG_OPERATION_SUBTRACT, { tools: four });
  const sliderBooleanMs = performance.now() - start;
  start = performance.now();
  const sliderTriangles = mesh(slider.body).triangles;
  const sliderTessellationMs = performance.now() - start;

  const result = {
    wall50BooleanMs, wall50TessellationMs, wall50Triangles,
    rotated20BooleanMs,
    sliderRebuildMs, sliderBooleanMs, sliderTessellationMs, sliderTriangles,
  };
  if (process.argv.includes('--check') && process.platform === 'darwin' && process.arch === 'arm64') {
    const baseline = JSON.parse(readFileSync(new URL('./performance-baseline.json', import.meta.url), 'utf8'));
    const timings = [
      'wall50BooleanMs', 'wall50TessellationMs', 'rotated20BooleanMs', 'sliderRebuildMs', 'sliderBooleanMs',
      'sliderTessellationMs',
    ];
    for (const key of timings) {
      if (result[key] > baseline[key] * 1.25) {
        throw new Error(`${key} regressed: ${result[key]} ms exceeds ${baseline[key] * 1.25} ms`);
      }
    }
  }
  console.log(JSON.stringify(result));
} finally {
  OpenGeometry.reset();
}
