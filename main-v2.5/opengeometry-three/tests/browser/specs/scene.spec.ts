import { expect, test } from '@playwright/test';
import { disposeFixture, type FixtureWindow } from '../support/acceptance-page.js';

type AcceptanceResult = {
  baseVolume: number;
  cutVolume: number;
  railVolume: number;
  rebuiltVolume: number;
  recutVolume: number;
  coverageGap: boolean;
  products: number;
  skipped: number;
  pcurvelessEdges: number;
};
type AcceptanceFixture = {
  backend: string;
  result: AcceptanceResult;
  rails: [{ record?: object }, ...{ record?: object }[]];
  exportStep(): Promise<{ text: string; report: { products: number } }>;
};

for (const backend of ['inline', 'worker'] as const) {
  test(`acceptance scene has expected geometry and STEP with ${backend}`, async ({ page }) => {
    test.setTimeout(40_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async (expected) => {
        const fixture = (window as FixtureWindow<AcceptanceFixture>).__ogTest;
        if (fixture.backend !== expected) throw new Error(`Expected ${expected}, got ${fixture.backend}`);
        const result = fixture.result;
        if (result.baseVolume !== 3.6 || result.cutVolume !== 3.222 || result.rebuiltVolume !== 4.8
          || result.recutVolume !== 4.422 || !result.coverageGap) {
          throw new Error('Scene volumes or CoverageGap changed');
        }
        if (result.products !== 12 || result.skipped !== 3 || result.pcurvelessEdges <= 0) {
          throw new Error('STEP counts changed');
        }
        if (!fixture.rails.every((rail) => rail.record === fixture.rails[0].record)) {
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
