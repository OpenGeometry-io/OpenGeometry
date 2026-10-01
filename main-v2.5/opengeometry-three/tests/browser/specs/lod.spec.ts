import { expect, test } from '@playwright/test';
import { disposeFixture, type FixtureWindow } from '../support/acceptance-page.js';

type RetryResult = {
  warning?: { code: string; bucket: number; retryBucket: number };
  bucket?: number;
  errors: unknown[];
};
type HysteresisResult = {
  p: number;
  lowEdge: number;
  belowLow: number;
  highEdge: number;
  aboveHigh: number;
  movingHigh: number;
  movingLow: number;
};
type HysteresisFixture = { lodHysteresisProbe(): HysteresisResult; orbitProbe(): Promise<{ jobs: number }> };

for (const backend of ['inline', 'worker'] as const) {
  test(`LimitExceeded retries one coarser bucket and reports a warning with ${backend}`, async ({ page }) => {
    test.setTimeout(45_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async () => {
        const fixture = (window as FixtureWindow<{ coarserRetryProbe(): Promise<RetryResult> }>).__ogTest;
        const result = await fixture.coarserRetryProbe();
        if (result.errors.length !== 0 || result.warning?.code !== 'LimitExceeded'
          || result.warning.retryBucket !== result.warning.bucket * 2 || result.bucket !== result.warning.retryBucket) {
          throw new Error(`Coarser retry failed: ${JSON.stringify(result)}`);
        }
      });
    } finally {
      await disposeFixture(page);
    }
  });
}

test('zoom selects a finer LOD bucket', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<{ lodProbe(): Promise<{ before: number; after: number }> }>).__ogTest;
      const result = await fixture.lodProbe();
      if (result.after > result.before / 2) {
        throw new Error(`LOD did not refine after 4x zoom: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('LOD hysteresis keeps stable buckets and an orbit queues at most one job', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<HysteresisFixture>).__ogTest;
      const values = fixture.lodHysteresisProbe();
      if (values.lowEdge !== values.p || values.belowLow >= values.p || values.highEdge !== values.p
        || values.aboveHigh <= values.p || values.movingHigh !== values.p || values.movingLow >= values.p) {
        throw new Error(`LOD hysteresis edges differ: ${JSON.stringify(values)}`);
      }
      const orbit = await fixture.orbitProbe();
      if (orbit.jobs > 1) throw new Error(`Orbit queued ${String(orbit.jobs)} jobs`);
    });
  } finally {
    await disposeFixture(page);
  }
});
