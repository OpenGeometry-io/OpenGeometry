import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page.js';

test('a missing worker script falls back inline, emits WorkerFailure and settles', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker&worker=missing');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    const result = await page.evaluate(() => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      return fixture.workerFailureProbe();
    });
    expect(result).toEqual({ failed: 0, hasRecord: true, errors: ['WorkerFailure'], backend: 'inline' });
  } finally {
    await disposeFixture(page);
  }
});

test('a real post-init worker crash restarts once, then falls back with the record intact and '
  + 'draws new bodies in the same render', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker&worker=crash');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    const result = await page.evaluate(() => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      return fixture.workerCrashProbe();
    });
    expect(result).toEqual({
      afterRestart: 'worker', afterFallback: 'inline', sameRecord: true, errors: ['WorkerFailure'],
      drawnInSameRender: true,
    });
  } finally {
    await disposeFixture(page);
  }
});

test('worker geometry event lets a render-on-demand app render again', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const result = await fixture.onDemandProbe();
      if (result.events < 1 || !result.appeared) {
        throw new Error('Render-on-demand body did not appear after geometry event');
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('rapid worker rebuilds keep the latest revision with at most two jobs', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const result = await fixture.staleWorkerProbe();
      if (result.displayed !== result.current || result.jobs > 2) {
        throw new Error(`Worker showed stale geometry or ran too many jobs: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('worker re-sends a missing snapshot and retries once', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      const result = await fixture.snapshotResendProbe();
      if (result.triangles <= 0) throw new Error('Snapshot retry produced no geometry');
    });
  } finally {
    await disposeFixture(page);
  }
});
