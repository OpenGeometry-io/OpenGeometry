import { expect, test } from '@playwright/test';
import { disposeFixture, type FixtureWindow } from '../support/acceptance-page.js';

type MemoryResult = { baseline: number; one: number; shared: number; unique: number; after: number };

test('instance geometry memory returns to baseline after disposal', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<{ memoryProbe(): Promise<MemoryResult> }>).__ogTest;
      const result = await fixture.memoryProbe();
      if (result.one <= result.baseline || result.shared !== result.one || result.unique !== result.shared + 1
        || result.after !== result.baseline) {
        throw new Error(`Geometry memory changed unexpectedly: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});
