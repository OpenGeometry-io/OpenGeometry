import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page.js';

test('transform updates a known pixel without changing geometry buffers', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const result = fixture.placementPixelProbe();
      if (!result.matrixMatches || !result.geometryMatches || !result.syncHit) {
        throw new Error('Transform changed geometry, matrix diverged or sync hit failed');
      }
      if ((result.before[2] ?? 0) <= (result.before[0] ?? 0) || result.after[0] !== 255 || result.after[1] !== 255
        || result.after[2] !== 255) throw new Error(`Known pixel did not move: ${JSON.stringify(result)}`);
    });
  } finally {
    await disposeFixture(page);
  }
});
