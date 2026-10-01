import { expect, test } from '@playwright/test';
import { disposeFixture, type FixtureWindow } from '../support/acceptance-page.js';

type PixelResult = {
  before: [number, number, number, number];
  after: [number, number, number, number];
  matrixMatches: boolean;
  geometryMatches: boolean;
  syncHit: boolean;
};

test('transform updates a known pixel without changing geometry buffers', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = (window as FixtureWindow<{ placementPixelProbe(): PixelResult }>).__ogTest;
      const result = fixture.placementPixelProbe();
      if (!result.matrixMatches || !result.geometryMatches || !result.syncHit) {
        throw new Error('Transform changed geometry, matrix diverged or sync hit failed');
      }
      if (result.before[2] <= result.before[0] || result.after[0] !== 255 || result.after[1] !== 255
        || result.after[2] !== 255) throw new Error(`Known pixel did not move: ${JSON.stringify(result)}`);
    });
  } finally {
    await disposeFixture(page);
  }
});
