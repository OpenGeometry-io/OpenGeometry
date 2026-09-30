import { expect, test, type Page } from '@playwright/test';
import BASELINE from '../../../../scripts/bench/performance-baseline.json' with { type: 'json' };

type FixtureWindow<T> = typeof window & { __ogTest: T };
type AcceptanceResult = {
  baseVolume: number;
  cutVolume: number;
  railVolume: number;
  rebuiltVolume: number;
  recutVolume: number;
  coverageGap: boolean;
  products: number;
  skipped: number;
  pcurvelessEdges: number;
};
type AcceptanceFixture = {
  backend: string;
  result: AcceptanceResult;
  rails: [{ record?: object }, ...{ record?: object }[]];
  exportStep(): Promise<{ text: string; report: { products: number } }>;
};
type StoreyFixture = {
  buildStorey(): Promise<{
    walls: number;
    openingsPerWall: number;
    instances: number;
    flushCount: number;
    renderFlushes: number;
    geometryCount: number;
  }>;
};
type StoreyPerformance = {
  transformFlushMs: number;
  stepMs: number;
  bytes: number;
  products: number;
  entities: number;
};
type StoreyProbeFixture = {
  buildStorey(): Promise<{ renderMs: number }>;
  storeyPerformanceProbe(): Promise<StoreyPerformance>;
};
type RetryResult = { calls: number; warning?: { code: string; bucket: number; retryBucket: number }; bucket?: number };
type TransactionResult = {
  threw: boolean;
  revived: boolean;
  dryResult: string;
  dryRestored: boolean;
  thenableCode: string;
};
type MemoryResult = { baseline: number; one: number; shared: number; unique: number; after: number };
type PixelResult = {
  before: [number, number, number, number];
  after: [number, number, number, number];
  matrixMatches: boolean;
  geometryMatches: boolean;
  syncHit: boolean;
};
type HysteresisResult = {
  p: number;
  lowEdge: number;
  belowLow: number;
  highEdge: number;
  aboveHigh: number;
  movingHigh: number;
  movingLow: number;
};
type HysteresisFixture = { lodHysteresisProbe(): HysteresisResult; orbitProbe(): Promise<{ jobs: number }> };
type CrashResult = { afterRestart: string; afterFallback: string; sameRecord: boolean };
type StaleResult = { current: number; displayed?: number; jobs: number };

const SAME_BASELINE_PLATFORM = process.platform === 'darwin' && process.arch === 'arm64';

async function disposeFixture(page: Page): Promise<void> {
  await page.evaluate(() => (window as typeof window & { __ogTest?: { dispose(): void } }).__ogTest?.dispose());
}

for (const backend of ['inline', 'worker'] as const) {
  test(`acceptance scene has expected geometry and STEP with ${backend}`, async ({ page }) => {
    test.setTimeout(40_000);
    await page.goto(`/acceptance.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
    try {
      await page.evaluate(async (expected) => {
        const fixture = (window as FixtureWindow<AcceptanceFixture>).__ogTest;
        if (fixture.backend !== expected) throw new Error(`Expected ${expected}, got ${fixture.backend}`);
        const result = fixture.result;
        if (result.baseVolume !== 3.6 || result.cutVolume !== 3.222 || result.rebuiltVolume !== 4.8
          || result.recutVolume !== 4.422 || !result.coverageGap) {
          throw new Error('Scene volumes or CoverageGap changed');
        }
        if (result.products !== 12 || result.skipped !== 3 || result.pcurvelessEdges <= 0) {
          throw new Error('STEP counts changed');
        }
        if (!fixture.rails.every((rail) => rail.record === fixture.rails[0].record)) {
          throw new Error('Rail instances do not share one record');
        }
        const second = await fixture.exportStep();
        if (second.report.products !== 12 || !second.text.includes('MANIFOLD_SOLID_BREP')) {
          throw new Error('STEP export changed');
        }
      }, backend);
    } finally {
      await disposeFixture(page);
    }
  });
}

test('storey scale scene has 200 walls, four openings each and 1000 instances', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<StoreyFixture>).__ogTest;
      const result = await fixture.buildStorey();
      if (result.walls !== 200 || result.openingsPerWall !== 4 || result.instances !== 1000) {
        throw new Error('Storey scene count changed');
      }
      if (result.geometryCount > 20) {
        throw new Error(`Storey scene uploaded too many geometries: ${String(result.geometryCount)}`);
      }
      if (result.renderFlushes > 1) {
        throw new Error(`Storey scene flushed ${String(result.renderFlushes)} times in one render`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('storey transform flush and STEP export stay within file limits', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    const result = await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<StoreyProbeFixture>).__ogTest;
      const built = await fixture.buildStorey();
      const measured = await fixture.storeyPerformanceProbe();
      if (measured.products !== 1200 || measured.entities > 2_000_000 || measured.bytes > 64 * 1024 * 1024) {
        throw new Error(`Storey export exceeded expected limits: ${JSON.stringify(measured)}`);
      }
      return { renderMs: built.renderMs, ...measured };
    });
    console.log(`Storey performance baseline: ${JSON.stringify(result)}`);
    if (SAME_BASELINE_PLATFORM) {
      expect(result.renderMs).toBeLessThanOrEqual(BASELINE.storeyFirstRenderMs * 1.25);
      expect(result.transformFlushMs).toBeLessThanOrEqual(BASELINE.storeyTransformFlushMs * 1.25);
      expect(result.stepMs).toBeLessThanOrEqual(BASELINE.storeyStepMs * 1.25);
    }
    expect(result.bytes).toBeLessThanOrEqual(BASELINE.storeyStepBytes * 1.25);
  } finally {
    await disposeFixture(page);
  }
});

test('LimitExceeded retries one coarser bucket and reports a warning', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = (window as FixtureWindow<{ coarserRetryProbe(): RetryResult }>).__ogTest;
      const result = fixture.coarserRetryProbe();
      if (result.calls !== 2 || result.warning?.code !== 'LimitExceeded'
        || result.warning.retryBucket !== result.warning.bucket * 2 || result.bucket !== result.warning.retryBucket) {
        throw new Error(`Coarser retry failed: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('nested transaction rollback revives a disposed body and dry run restores placement', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = (window as FixtureWindow<{ transactionProbe(): TransactionResult }>).__ogTest;
      const result = fixture.transactionProbe();
      if (!result.threw || !result.revived || result.dryResult !== 'trial' || !result.dryRestored
        || result.thenableCode !== 'InvalidParameter') {
        throw new Error(`Transaction behavior changed: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('instance geometry memory returns to baseline after disposal', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<{ memoryProbe(): Promise<MemoryResult> }>).__ogTest;
      const result = await fixture.memoryProbe();
      if (result.one <= result.baseline || result.shared !== result.one || result.unique !== result.shared + 1
        || result.after !== result.baseline) {
        throw new Error(`Geometry memory changed unexpectedly: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

test('zoom selects a finer LOD bucket', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<{ lodProbe(): Promise<{ before: number; after: number }> }>).__ogTest;
      const result = await fixture.lodProbe();
      if (result.after > result.before / 2) {
        throw new Error(`LOD did not refine after 4x zoom: ${JSON.stringify(result)}`);
      }
    });
  } finally {
    await disposeFixture(page);
  }
});

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

test('transform updates a known pixel without changing geometry buffers', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=inline');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(() => {
      const fixture = (window as FixtureWindow<{ placementPixelProbe(): PixelResult }>).__ogTest;
      const result = fixture.placementPixelProbe();
      if (!result.matrixMatches || !result.geometryMatches || !result.syncHit) {
        throw new Error('Transform changed geometry, matrix diverged or sync hit failed');
      }
      if (result.before[2] <= result.before[0] || result.after[0] !== 255 || result.after[1] !== 255
        || result.after[2] !== 255) throw new Error(`Known pixel did not move: ${JSON.stringify(result)}`);
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

test('LOD hysteresis keeps stable buckets and an orbit queues at most one job', async ({ page }) => {
  test.setTimeout(45_000);
  await page.goto('/acceptance.html?backend=worker');
  await expect(page.locator('#status')).toHaveText('Acceptance ready', { timeout: 20_000 });
  try {
    await page.evaluate(async () => {
      const fixture = (window as FixtureWindow<HysteresisFixture>).__ogTest;
      const values = fixture.lodHysteresisProbe();
      if (values.lowEdge !== values.p || values.belowLow >= values.p || values.highEdge !== values.p
        || values.aboveHigh <= values.p || values.movingHigh !== values.p || values.movingLow >= values.p) {
        throw new Error(`LOD hysteresis edges differ: ${JSON.stringify(values)}`);
      }
      const orbit = await fixture.orbitProbe();
      if (orbit.jobs > 1) throw new Error(`Orbit queued ${String(orbit.jobs)} jobs`);
    });
  } finally {
    await disposeFixture(page);
  }
});
