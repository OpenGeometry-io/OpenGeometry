import { expect, test } from '@playwright/test';
import { EXAMPLES_URL } from '../support/servers';

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
      const example: unknown = Reflect.get(window, '__ogAnalyticBooleanExample');
      const current: unknown = typeof example === 'object' && example !== null
        ? Reflect.get(example, 'current') : undefined;
      const faces: unknown = typeof current === 'object' && current !== null
        ? Reflect.get(current, 'faces') : undefined;
      return typeof faces === 'number' ? faces : 0;
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

test('rounded section example reports its faces and volume', async ({ page }) => {
  await page.goto(`${EXAMPLES_URL}/shapes/rounded-profile.html`);
  const status = page.locator('#status');
  await expect(status).toContainText('Ready');
  await expect(page.locator('#error')).toBeEmpty();
  const volume = async (): Promise<number> => Number(/kernel ([\d.]+) m³/.exec(await status.innerText())?.[1]);
  await expect(status).toContainText('10 faces');
  expect(Math.abs(await volume() - (8 - (1 - Math.PI / 4)) * 3)).toBeLessThan(1e-6);

  await page.locator('[data-control="radius"] [data-number]').fill('0');
  await expect(status).toContainText('6 faces');
  expect(Math.abs(await volume() - 24)).toBeLessThan(1e-6);
  await expect(page.locator('#error')).toBeEmpty();
});

test('arc path sweep example reports its faces and the kernel volume', async ({ page }) => {
  await page.goto(`${EXAMPLES_URL}/operations/sweep-arc-path.html`);
  const status = page.locator('#status');
  await expect(status).toContainText('Ready');
  await expect(page.locator('#error')).toBeEmpty();
  const volume = async (): Promise<number> => Number(/kernel ([\d.]+) m³/.exec(await status.innerText())?.[1]);
  const expected = (radius: number, degrees: number): number => 0.33 * (4 + radius * degrees * Math.PI / 180);
  await expect(status).toContainText('14 faces (2 cylinders)');
  expect(Math.abs(await volume() / expected(1, 90) - 1)).toBeLessThan(1e-3);

  await page.locator('[data-control="angle"] [data-number]').fill('180');
  await expect(status).toContainText('path 7.141593 m');
  await expect(status).toContainText('14 faces (2 cylinders)');
  expect(Math.abs(await volume() / expected(1, 180) - 1)).toBeLessThan(1e-3);

  await page.locator('[data-control="radius"] [data-number]').fill('0.5');
  await expect(status).toContainText('Sweep refused');
  await expect(page.locator('#error')).toContainText('bend radius is within the profile reach');
});

test('curved meets straight example reports a valid union with the summed volume', async ({ page }) => {
  await page.goto(`${EXAMPLES_URL}/operations/curved-meets-straight.html`);
  const status = page.locator('#status');
  await expect(status).toContainText('Ready');
  await expect(page.locator('#error')).toBeEmpty();
  await expect(status).toContainText('valid true · closed true');
  const volume = async (): Promise<number> => Number(/kernel ([\d.]+) m³/.exec(await status.innerText())?.[1]);
  const text = async (): Promise<string> => status.innerText();
  const segments = async (): Promise<number> => Number(/(\d+) outline segments/.exec(await text())?.[1]);
  const quarter = Math.PI / 4 * (3.1 ** 2 - 2.9 ** 2);
  const overlap = (reach: number): number =>
    (reach * Math.sqrt(3.1 ** 2 - reach ** 2) + 3.1 ** 2 * Math.asin(reach / 3.1)) / 2 - 2.9 * reach;
  const expected = (reach: number): number =>
    3 * (quarter + 0.2 * (2 + reach) + 0.25 * 2 - overlap(reach) - overlap(0.05));
  expect(Math.abs(await volume() / expected(0.5) - 1)).toBeLessThan(1e-3);
  expect(await segments()).toBeGreaterThan(0);

  await page.locator('[data-control="reach"] [data-number]').fill('0.9');
  await expect(status).toContainText(`volume ${expected(0.9).toFixed(6)} m³`);
  await expect(status).toContainText('valid true · closed true');
  expect(Math.abs(await volume() / expected(0.9) - 1)).toBeLessThan(1e-3);
  await expect(page.locator('#error')).toBeEmpty();
});
