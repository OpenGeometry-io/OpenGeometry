import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page.js';

for (const backend of ['inline', 'worker'] as const) {
  test(`acceptance scene has expected geometry and STEP with ${backend}`, async ({ page }) => {
    test.setTimeout(40_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async (expected) => {
        const fixture = window.ogAcceptance;
        if (!fixture) throw new Error('The acceptance page published no fixture');
        if (fixture.backend !== expected) throw new Error(`Expected ${expected}, got ${fixture.backend}`);
        const result = fixture.result;
        const near = (actual: number, target: number, tolerance: number): boolean =>
          Math.abs(actual - target) <= tolerance;
        if (!near(result.baseVolume, 3.6, 1e-5) || !near(result.cutVolume, 3.222, 1e-5)
          || !near(result.rebuiltVolume, 4.8, 1e-5) || !near(result.recutVolume, 4.422, 1e-5)
          || !near(result.railVolume, Math.PI * 0.05 ** 2 * 7, 5e-5) || !result.coverageGap) {
          throw new Error(`Scene volumes or CoverageGap changed: ${JSON.stringify(result)}`);
        }
        const railBounds = [0.95, 3.95, -4.05, 4.0, 4.05, 0];
        if (!result.railBounds.every((value, index) => near(value, railBounds[index] ?? Number.NaN, 1e-6))
          || !near(result.wallBounds[1], 3, 1e-6)) {
          throw new Error(`Placed bounds changed: ${JSON.stringify([result.railBounds, result.wallBounds])}`);
        }
        if (result.products !== 12 || result.skipped !== 3 || result.pcurvelessEdges <= 0) {
          throw new Error('STEP counts changed');
        }
        const rails = fixture.rails;
        if (!rails.every((rail) => rail.record !== undefined && rail.record === rails[0]?.record)) {
          throw new Error('Rail instances do not share one record');
        }
        const second = await fixture.exportStep();
        if (second.report.products !== 12 || !second.text.includes('MANIFOLD_SOLID_BREP')) {
          throw new Error('STEP export changed');
        }
      }, backend);
    } finally {
      await disposeFixture(page);
    }
  });
}
