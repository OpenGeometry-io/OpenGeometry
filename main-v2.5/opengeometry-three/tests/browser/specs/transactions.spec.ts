import { expect, test } from '@playwright/test';
import { disposeFixture, type FixtureWindow } from '../support/acceptance-page.js';

type TransactionResult = {
  threw: boolean;
  revived: boolean;
  dryResult: string;
  dryRestored: boolean;
  thenableCode: string;
};

test('nested transaction rollback revives a disposed body and dry run restores placement', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = (window as FixtureWindow<{ transactionProbe(): TransactionResult }>).__ogTest;
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
