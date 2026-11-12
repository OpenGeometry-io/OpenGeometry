import { expect, test } from '@playwright/test';
import type { ReleaseIdleResult } from '../support/fixture-types';

test('a released pool keeps no dispose listener of a disposed renderer on idle records', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/release-idle.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Release idle ready', { timeout: 20_000 });
  const result = await page.evaluate(async (): Promise<ReleaseIdleResult> => {
    const fixture = window.ogReleaseIdle;
    if (!fixture) throw new Error('The release idle page published no probe');
    return await fixture.releaseIdleProbe();
  });
  expect(result.idle).toBeGreaterThan(0);
  expect(result.idleWithListeners).toBe(result.idle);
  expect(result.released).toEqual({ records: result.idle, bytes: result.idleBytes });
  expect(result.idleAfter).toBe(0);
  expect(result.releasedWithListeners).toBe(0);
});
