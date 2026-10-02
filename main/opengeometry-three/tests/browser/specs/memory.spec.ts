import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page';

test('instance geometry memory returns to baseline after disposal', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const result = await fixture.memoryProbe();
      if (result.one <= result.baseline || result.shared !== result.one || result.unique !== result.shared + 1
        || result.after !== result.baseline) {
        throw new Error(`Geometry memory changed unexpectedly: ${JSON.stringify(result)}`);
      }
      if (result.outlineOne - result.after !== 2 * (result.one - result.baseline)
        || result.outlineShared !== result.outlineOne
        || result.outlineUnique - result.outlineShared !== 2 * (result.unique - result.shared)
        || result.outlineAfter !== result.baseline) {
        throw new Error(`Outline geometry memory changed unexpectedly: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});
