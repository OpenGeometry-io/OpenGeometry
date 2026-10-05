import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
import { OpenGeometry, OG_OPERATION_EXTRUDE, OG_OPERATION_SUBTRACT } from '../../dist/index.js';
import { createInlineKernel } from '../lib/kernel.mjs';
import { budgetFindings, median } from './budgets.mjs';
import { cutters, mesh, mitredWall, wall } from './scenes.mjs';

const RUNS = 5;
const MITRED_CUTS = 20;
const MITRED_PRISM_FACES = 6;
const FACES_PER_OPENING = 4;
const SERIAL_HANDLER = 'subtract_planar_polyhedra';

await createInlineKernel();
const MEDIANS = measured();
console.log(JSON.stringify(MEDIANS));
if (process.argv.includes('--check')) checkBudgets(MEDIANS);

function measured() {
  const sizes = {};
  const timings = {};
  for (const scene of [wall50, rotated20, mitred20, slider]) {
    const runs = repeated(scene);
    Object.assign(sizes, medians(runs.map((run) => run.sizes)));
    Object.assign(timings, medians(runs.map((run) => run.timings)));
  }
  return { sizes, timings };
}

function checkBudgets(medianResult) {
  const baseline = JSON.parse(readFileSync(new URL('./performance-baseline.json', import.meta.url), 'utf8'));
  const ci = process.env.CI === 'true';
  const key = `${process.platform}-${process.arch}`;
  const { findings, notes } = budgetFindings(medianResult, baseline, key, ci);
  for (const note of notes) console.log(note);
  if (findings.length > 0) throw new Error(findings.join('\n'));
}

function repeated(scene) {
  const runs = [];
  for (let run = 0; run < RUNS; run += 1) {
    try {
      runs.push(scene());
    } finally {
      OpenGeometry.reset();
    }
  }
  return runs;
}

function medians(records) {
  const names = Object.keys(records[0] ?? {});
  return Object.fromEntries(names.map((name) => [name, median(records.map((record) => record[name]))]));
}

function timed(action) {
  const start = performance.now();
  const value = action();
  return { value, ms: performance.now() - start };
}

function wall50() {
  const host = wall('straight', 60);
  const tools = cutters(50, 'straight', 60, 1.1);
  const boolean = timed(() => host.body.operate(OG_OPERATION_SUBTRACT, { tools }));
  const tessellation = timed(() => mesh(host.body).triangles);
  return {
    sizes: { wall50Triangles: tessellation.value },
    timings: { wall50BooleanMs: boolean.ms, wall50TessellationMs: tessellation.ms },
  };
}

function rotated20() {
  const host = wall('angled', 30);
  host.body.transform('Rotate', { axis: [0, 1, 0], degrees: 30, pivot: [0, 0, 0] });
  const tools = cutters(20, 'angled', 30, 1.35, 30);
  const boolean = timed(() => host.body.operate(OG_OPERATION_SUBTRACT, { tools }));
  return { sizes: {}, timings: { rotated20BooleanMs: boolean.ms } };
}

function mitred20() {
  const host = mitredWall('mitred', 30);
  const tools = cutters(MITRED_CUTS, 'mitred', 30, 1.35);
  const boolean = timed(() => host.operate(OG_OPERATION_SUBTRACT, { tools }));
  checkMitredOpenings(host);
  return { sizes: { mitred20Triangles: mesh(host).triangles }, timings: { mitred20BooleanMs: boolean.ms } };
}

function checkMitredOpenings(host) {
  const handlers = host.getReport()?.handlers ?? [];
  const faces = host.getBrep().topology.faces.length;
  const expectedFaces = MITRED_PRISM_FACES + FACES_PER_OPENING * MITRED_CUTS;
  if (faces === expectedFaces && handlers.length === MITRED_CUTS && handlers.every((name) => name === SERIAL_HANDLER)) {
    return;
  }
  throw new Error(
    `mitred wall openings changed: expected ${String(MITRED_CUTS)} ${SERIAL_HANDLER} cuts and `
    + `${String(expectedFaces)} faces, got ${String(faces)} faces and handlers ${JSON.stringify(handlers)}`,
  );
}

function slider() {
  const host = wall('slider', 6);
  const tools = cutters(4, 'slider', 6, 1.2);
  const rebuild = timed(() => host.body.rebuild(OG_OPERATION_EXTRUDE, { profile: host.profile, distance: 4 }));
  const boolean = timed(() => host.body.operate(OG_OPERATION_SUBTRACT, { tools }));
  const tessellation = timed(() => mesh(host.body).triangles);
  return {
    sizes: { sliderTriangles: tessellation.value },
    timings: { sliderRebuildMs: rebuild.ms, sliderBooleanMs: boolean.ms, sliderTessellationMs: tessellation.ms },
  };
}
