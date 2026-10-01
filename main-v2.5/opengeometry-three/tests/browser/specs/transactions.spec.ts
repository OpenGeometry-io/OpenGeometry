import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page.js';

test('nested transaction rollback revives a disposed body and dry run restores placement', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const result = fixture.transactionProbe();
      if (!result.threw || !result.revived || result.dryResult !== 'trial' || !result.dryRestored
        || result.thenableCode !== 'InvalidParameter') {
        throw new Error(`Transaction behavior changed: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

for (const backend of ['inline', 'worker'] as const) {
  test(`dispose, reuse the ogId and rollback revive the old object with ${backend}`, async ({ page }) => {
    test.setTimeout(45_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async () => {
        const fixture = window.ogAcceptance;
        if (!fixture) throw new Error('The acceptance page published no fixture');
        const result = await fixture.reuseProbe();
        if (!result.revived || !result.gone || result.errors.length !== 0) {
          throw new Error(`Reuse rollback failed: ${JSON.stringify(result)}`);
        }
      });
    } finally {
      await disposeFixture(page);
    }
  });
}
