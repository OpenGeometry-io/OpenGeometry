import { expect, test } from '@playwright/test';
import BASELINE from '../../../../scripts/bench/performance-baseline.json' with { type: 'json' };
import { disposeFixture } from '../support/acceptance-page';

const SAME_BASELINE_PLATFORM = process.platform === 'darwin' && process.arch === 'arm64';
const GEOMETRY_FLOOR = 100;
const GEOMETRY_BOUND = 600;

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
    const result = await page.evaluate(async () => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const built = await fixture.buildStorey();
      const measured = await fixture.storeyPerformanceProbe();
      if (measured.products !== 1200 || measured.entities > 2_000_000 || measured.bytes > 64 * 1024 * 1024) {
        throw new Error(`Storey export exceeded expected limits: ${JSON.stringify(measured)}`);
      }
      return { renderMs: built.renderMs, ...measured };
    });
    console.log(`Storey performance baseline: ${JSON.stringify(result)}`);
    if (SAME_BASELINE_PLATFORM) {
      expect(result.renderMs).toBeLessThanOrEqual(BASELINE.storeyFirstRenderMs * 1.25);
      expect(result.transformFlushMs).toBeLessThanOrEqual(BASELINE.storeyTransformFlushMs * 1.25);
      expect(result.stepMs).toBeLessThanOrEqual(BASELINE.storeyStepMs * 1.25);
    }
    expect(result.bytes).toBeLessThanOrEqual(BASELINE.storeyStepBytes * 1.25);
  } finally {
    await disposeFixture(page);
  }
});
