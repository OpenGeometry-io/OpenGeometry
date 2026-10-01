import { expect, test } from '@playwright/test';
import { EXAMPLES_URL } from '../support/servers.js';

type AnalyticExample = { current: { faces: number } };

test('source pages explain how to start the local server when opened as files', async ({ page }) => {
  for (const source of ['../../../examples-vite/index.html', '../pages/acceptance.html']) {
    await page.goto(new URL(source, import.meta.url).href);
    await expect(page.locator('#status')).toContainText('Use the local Vite server');
  }
});

for (const backend of ['inline', 'worker'] as const) {
  test(`analytic Boolean example updates without errors with ${backend}`, async ({ page }) => {
    await page.goto(`${EXAMPLES_URL}/operations/analytic-boolean-operations.html?backend=${backend}`);
    await expect(page.locator('#status')).toContainText('Ready');
    await expect(page.locator('#report')).toContainText('subtraction · analytic');
    await expect(page.locator('#error')).toBeEmpty();
    expect(await page.locator('#app canvas').evaluate(
      (canvas: HTMLCanvasElement) => canvas.width > 0 && canvas.height > 0,
    )).toBe(true);

    await page.locator('#offset').evaluate((element: HTMLInputElement) => {
      element.value = '0';
      element.dispatchEvent(new globalThis.Event('input', { bubbles: true }));
    });
    await expect(page.locator('#report')).toContainText('0 result faces');
    await expect(page.locator('#error')).toBeEmpty();

    await page.locator('#operation').selectOption('intersection');
    await page.locator('#offset').evaluate((element: HTMLInputElement) => {
      element.value = '2.5';
      element.dispatchEvent(new globalThis.Event('input', { bubbles: true }));
    });
    await expect(page.locator('#report')).toContainText('intersection · analytic');
    await expect(page.locator('#report')).toContainText('0 result faces');
    await expect(page.locator('#error')).toBeEmpty();

    await page.locator('#operation').selectOption('union');
    await page.locator('#offset').evaluate((element: HTMLInputElement) => {
      element.value = '1';
      element.dispatchEvent(new globalThis.Event('input', { bubbles: true }));
    });
    await expect(page.locator('#report')).toContainText('union · analytic');
    await expect.poll(async () => page.evaluate(() => {
      const example = (window as typeof window & { __ogAnalyticBooleanExample: AnalyticExample })
        .__ogAnalyticBooleanExample;
      return example.current.faces;
    })).toBeGreaterThan(0);
    await expect(page.locator('#error')).toBeEmpty();

    await page.locator('#deflection').evaluate((element: HTMLInputElement) => {
      element.value = '-1.5';
      element.dispatchEvent(new globalThis.Event('input', { bubbles: true }));
    });
    await expect(page.locator('#status')).toContainText('Ready · mesh');
    await expect(page.locator('#error')).toBeEmpty();
  });
}
