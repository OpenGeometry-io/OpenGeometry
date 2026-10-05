import { expect, test } from '@playwright/test';
import BASELINE from '../../../../scripts/bench/performance-baseline.json' with { type: 'json' };
import { disposeFixture } from '../support/acceptance-page';

const GEOMETRY_FLOOR = 100;
const GEOMETRY_BOUND = 600;
const PROBE_RUNS = 5;
const MARGIN = 1.25;

type TimingBlock = (typeof BASELINE.timings)[keyof typeof BASELINE.timings];

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
}

function timingBaseline(): TimingBlock | undefined {
  if (process.env.CI === 'true') return undefined;
  return new Map(Object.entries(BASELINE.timings)).get(`${process.platform}-${process.arch}`);
}

for (const backend of ['inline', 'worker'] as const) {
  test(
    `storey has 200 distinct walls with four measured openings each and 1000 instances with ${backend}`,
    async ({ page }) => {
      test.setTimeout(120_000);
      await page.goto(`/acceptance.html?backend=${backend}`);
      await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
      try {
        const measured = await page.evaluate(async (geometry) => {
          const fixture = window.ogAcceptance;
          if (!fixture) throw new Error('The acceptance page published no fixture');
          const started = performance.now();
          const result = await fixture.buildStorey();
          const buildMs = performance.now() - started;
          if (result.walls !== 200 || result.uniqueWalls !== 200 || result.instances !== 1000) {
            throw new Error(`Storey scene counts changed: ${JSON.stringify(result)}`);
          }
          const { min, max } = result.openingsPerWall;
          if (min !== max || min !== 4) throw new Error(`Storey walls have ${String(min)} to ${String(max)} openings`);
          if (result.renderFlushes > 1) {
            throw new Error(`Storey scene flushed ${String(result.renderFlushes)} times in one render`);
          }
          if (result.geometryCount < geometry.floor || result.geometryCount > geometry.bound) {
            throw new Error(`Storey scene uploaded ${String(result.geometryCount)} geometries`);
          }
          if (result.failed !== 0) {
            throw new Error(`Storey scene left ${String(result.failed)} bodies without current geometry`);
          }
          return { geometryCount: result.geometryCount, buildMs };
        }, { floor: GEOMETRY_FLOOR, bound: GEOMETRY_BOUND });
        console.log(`Storey scene with ${backend}: ${JSON.stringify(measured)}`);
      } finally {
        await disposeFixture(page);
      }
    },
  );
}

test('storey transform flush and STEP export stay within file limits', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    const probed = await page.evaluate(async (runs) => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const built = await fixture.buildStorey();
      let measured = await fixture.storeyPerformanceProbe();
      const flushRuns = [measured.transformFlushMs];
      const stepRuns = [measured.stepMs];
      for (let run = 1; run < runs; run += 1) {
        measured = await fixture.storeyPerformanceProbe();
        flushRuns.push(measured.transformFlushMs);
        stepRuns.push(measured.stepMs);
      }
      if (measured.products !== 1200 || measured.entities > 2_000_000 || measured.bytes > 64 * 1024 * 1024) {
        throw new Error(`Storey export exceeded expected limits: ${JSON.stringify(measured)}`);
      }
      return { renderMs: built.renderMs, ...measured, flushRuns, stepRuns };
    }, PROBE_RUNS);
    const result = { ...probed, transformFlushMs: median(probed.flushRuns), stepMs: median(probed.stepRuns) };
    console.log(`Storey performance baseline: ${JSON.stringify(result)}`);
    const timings = timingBaseline();
    if (timings) {
      expect(result.renderMs).toBeLessThanOrEqual(timings.storeyFirstRenderMs * MARGIN);
      expect(result.transformFlushMs).toBeLessThanOrEqual(timings.storeyTransformFlushMs * MARGIN);
      expect(result.stepMs).toBeLessThanOrEqual(timings.storeyStepMs * MARGIN);
    }
    expect(result.bytes).toBeLessThanOrEqual(BASELINE.sizes.storeyStepBytes * MARGIN);
    expect(result.entities).toBeLessThanOrEqual(BASELINE.sizes.storeyStepEntities * MARGIN);
  } finally {
    await disposeFixture(page);
  }
});
