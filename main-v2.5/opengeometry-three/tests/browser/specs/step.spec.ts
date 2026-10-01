import { expect, test } from '@playwright/test';
import { disposeFixture } from '../support/acceptance-page';
import { part21Check, writeStep } from '../support/step-output';

const LEVEL_EXPORTS = [
  { name: 'level-mm-z', unit: 'millimetre', upAxis: 'Z' },
  { name: 'level-m-y', unit: 'metre', upAxis: 'Y' },
] as const;
const STOREY_NODES = [
  ...Array.from({ length: 200 }, (_, index) => `storey-wall-${String(index)}`),
  ...Array.from({ length: 1000 }, (_, index) => `storey-rail-${String(index)}`),
];

async function expectOracle(args: string[], log: string): Promise<void> {
  const started = performance.now();
  const status = await part21Check(args, log);
  const elapsed = Math.round(performance.now() - started);
  console.log(`part21_check ${args.join(' ')}: exit ${String(status)} in ${String(elapsed)} ms`);
  expect(status, `part21_check failed, see ${log}`).toBe(0);
}

for (const backend of ['inline', 'worker'] as const) {
  const title = `acceptance STEP passes the Part-21 oracle and equals the native scene with ${backend}`;
  test(title, async ({ page }, testInfo) => {
    test.setTimeout(600_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      for (const { name, unit, upAxis } of LEVEL_EXPORTS) {
        const exported = await page.evaluate(async (options) => {
          const fixture = window.ogAcceptance;
          if (!fixture) throw new Error('The acceptance page published no fixture');
          if (fixture.backend !== options.backend) {
            throw new Error(`Expected ${options.backend}, got ${fixture.backend}`);
          }
          return fixture.exportStep({ unit: options.unit, upAxis: options.upAxis });
        }, { backend, unit, upAxis });
        const files = await writeStep(testInfo, name, exported);
        await expectOracle(['acceptance', files.step, files.report, unit, upAxis], files.log);
      }
    } finally {
      await disposeFixture(page);
    }
  });
}

test('storey STEP passes the Part-21 oracle', async ({ page }, testInfo) => {
  test.setTimeout(600_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    const exported = await page.evaluate(async (nodes) => {
      const fixture = window.ogAcceptance;
      if (!fixture) throw new Error('The acceptance page published no fixture');
      await fixture.buildStorey();
      return fixture.exportStep({ nodes });
    }, STOREY_NODES);
    const files = await writeStep(testInfo, 'storey', exported);
    await expectOracle(['storey', files.step, files.report], files.log);
  } finally {
    await disposeFixture(page);
  }
});
