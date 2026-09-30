import { expect, test } from '@playwright/test';
import { disposeFixture, type FixtureWindow } from '../support/acceptance-page.js';

type CrashResult = { afterRestart: string; afterFallback: string; sameRecord: boolean };
type StaleResult = { current: number; displayed?: number; jobs: number };

test('worker crash restarts once and then falls back with the record intact', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<{ workerCrashProbe(): Promise<CrashResult> }>).__ogTest;
      const result = await fixture.workerCrashProbe();
      if (result.afterRestart !== 'worker' || result.afterFallback !== 'inline' || !result.sameRecord) {
        throw new Error(`Worker recovery failed: ${JSON.stringify(result)}`);
      }
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
      const fixture = (window as FixtureWindow<{ onDemandProbe(): Promise<{ events: number; appeared: boolean }> }>)
        .__ogTest;
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
      const fixture = (window as FixtureWindow<{ staleWorkerProbe(): Promise<StaleResult> }>).__ogTest;
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
      const fixture = (window as FixtureWindow<{ snapshotResendProbe(): Promise<{ triangles: number }> }>).__ogTest;
      const result = await fixture.snapshotResendProbe();
      if (result.triangles <= 0) throw new Error('Snapshot retry produced no geometry');
    });
  } finally {
    await disposeFixture(page);
  }
});
