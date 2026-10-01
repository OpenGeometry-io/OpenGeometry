import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page.js';

for (const backend of ['inline', 'worker'] as const) {
  test(`LimitExceeded retries one coarser bucket and reports a warning with ${backend}`, async ({ page }) => {
    test.setTimeout(45_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async () => {
        const fixture = window.ogAcceptance;
        if (!fixture) throw new Error('The acceptance page published no fixture');
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

for (const backend of ['inline', 'worker'] as const) {
  test(`zoom selects a finer LOD bucket with ${backend}`, async ({ page }) => {
    test.setTimeout(45_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async () => {
        const fixture = window.ogAcceptance;
        if (!fixture) throw new Error('The acceptance page published no fixture');
        const result = await fixture.lodProbe();
        if (result.after > result.before / 2) {
          throw new Error(`LOD did not refine after 4x zoom: ${JSON.stringify(result)}`);
        }
      });
    } finally {
      await disposeFixture(page);
    }
  });
}

test('LOD hysteresis keeps stable buckets and an orbit queues at most one job', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const values = await fixture.lodHysteresisProbe();
      const [kept, raised, held, lowered] = values.buckets;
      if (kept !== values.p || raised !== 4 * values.p || held !== raised
        || lowered === undefined || lowered >= raised) {
        throw new Error(`LOD hysteresis edges differ: ${JSON.stringify(values)}`);
      }
      const orbit = await fixture.orbitProbe();
      const start = orbit.buckets[0] ?? 0;
      const coarsened = orbit.buckets.some((bucket, index) => bucket > (orbit.buckets[index - 1] ?? bucket))
        || orbit.sendBuckets.some((bucket) => bucket > start);
      if (orbit.jobs > 1 || coarsened) throw new Error(`Orbit coarsened or queued jobs: ${JSON.stringify(orbit)}`);
    });
  } finally {
    await disposeFixture(page);
  }
});
