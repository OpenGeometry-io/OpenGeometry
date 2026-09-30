import { defineConfig } from '@playwright/test';
import { EXAMPLES_URL, PAGES_URL, PORT_BASE } from './opengeometry-three/tests/browser/support/servers.js';

export default defineConfig({
  testDir: './opengeometry-three/tests/browser/specs',
  workers: 1,
  fullyParallel: false,
  retries: 0,
  outputDir: `test-results/${String(PORT_BASE)}`,
  timeout: 30_000,
  use: { browserName: 'chromium', headless: true, baseURL: PAGES_URL },
  webServer: [
    {
      command: `npm run dev-example -- --host 127.0.0.1 --port ${new URL(EXAMPLES_URL).port} --strictPort`,
      url: EXAMPLES_URL,
      reuseExistingServer: false,
      timeout: 30_000,
    },
    {
      command: `npm run dev-test-pages -- --host 127.0.0.1 --port ${new URL(PAGES_URL).port} --strictPort`,
      url: `${PAGES_URL}/smoke.html`,
      reuseExistingServer: false,
      timeout: 30_000,
    },
  ],
});
