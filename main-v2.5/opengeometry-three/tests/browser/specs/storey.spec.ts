import { expect, test } from '@playwright/test';
import BASELINE from '../../../../scripts/bench/performance-baseline.json' with { type: 'json' };
import { disposeFixture } from '../support/acceptance-page';

const SAME_BASELINE_PLATFORM = process.platform === 'darwin' && process.arch === 'arm64';

for (const backend of ['inline', 'worker'] as const) {
  test(`storey scale scene has 200 walls, four openings each and 1000 instances with ${backend}`, async ({ page }) => {
    test.setTimeout(120_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async () => {
        const fixture = window.ogAcceptance;
        if (!fixture) throw new Error('The acceptance page published no fixture');
        const result = await fixture.buildStorey();
        if (result.walls !== 200 || result.openingsPerWall !== 4 || result.instances !== 1000) {
          throw new Error('Storey scene count changed');
        }
        if (result.geometryCount > 20) {
          throw new Error(`Storey scene uploaded too many geometries: ${String(result.geometryCount)}`);
        }
        if (result.renderFlushes > 1) {
          throw new Error(`Storey scene flushed ${String(result.renderFlushes)} times in one render`);
        }
      });
    } finally {
      await disposeFixture(page);
    }
  });
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
