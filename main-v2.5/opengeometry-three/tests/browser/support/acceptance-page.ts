import type { Page } from '@playwright/test';

export type FixtureWindow<T> = typeof window & { __ogTest: T };

export async function disposeFixture(page: Page): Promise<void> {
  await page.evaluate(() => (window as typeof window & { __ogTest?: { dispose(): void } }).__ogTest?.dispose());
}
