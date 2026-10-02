import { expect, test } from '@playwright/test';

function checkQuickStart(): void {
  const fixture = window.ogQuickStart;
  if (!fixture) throw new Error('The quick start page published no fixture');
  if (fixture.errors.length) throw new Error(`The SDK emitted errors: ${JSON.stringify(fixture.errors)}`);
  if (fixture.backend !== 'worker') throw new Error(`Expected worker, got ${fixture.backend}`);
  if (fixture.settled.failed.length || !fixture.wall.record) throw new Error('The wall did not settle');
  const { products, solids, faces } = fixture.report;
  if (products !== 1 || solids !== 1 || faces !== 22) {
    throw new Error(`STEP counts differ: ${JSON.stringify({ products, solids, faces })}`);
  }
  const bounds = fixture.bounds;
  const near = (actual: number, target: number): boolean => Math.abs(actual - target) <= 1e-6;
  if (!bounds || !near(bounds[0], -3) || !near(bounds[3], 3) || !near(bounds[1], 0) || !near(bounds[4], 3)) {
    throw new Error(`The wall bounds differ: ${JSON.stringify(bounds)}`);
  }
}

function countDrawnPixels(): number {
  const fixture = window.ogQuickStart;
  if (!fixture) throw new Error('The quick start page published no fixture');
  const canvas = document.createElement('canvas');
  canvas.width = fixture.renderer.domElement.width;
  canvas.height = fixture.renderer.domElement.height;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('The 2D canvas context is unavailable');
  context.drawImage(fixture.renderer.domElement, 0, 0);
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
  let drawn = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    const black = pixels[index] === 0 && pixels[index + 1] === 0 && pixels[index + 2] === 0;
    if (!black || pixels[index + 3] !== 255) drawn++;
  }
  return drawn;
}

test('the README quick start boots from URLs, draws the wall and exports it', async ({ page }) => {
  const problems: string[] = [];
  page.on('pageerror', (error) => { problems.push(error.message); });
  page.on('console', (message) => { if (message.type() === 'error') problems.push(message.text()); });
  await page.goto('/quick-start.html');
  try {
    await expect(page.locator('#status')).toHaveText('Quick start ready');
    await page.evaluate(checkQuickStart);
    expect(await page.evaluate(countDrawnPixels)).toBeGreaterThan(1_000);
    expect(problems).toEqual([]);
  } finally {
    await page.evaluate(() => { window.ogQuickStart?.dispose(); });
  }
});
