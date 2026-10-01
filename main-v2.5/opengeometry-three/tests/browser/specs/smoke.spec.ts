import { expect, test } from '@playwright/test';

type SmokeRecord = {
  revision: number;
  bucket: number;
  triangles: number;
  faceRanges: Uint32Array;
  edgeIds: Uint32Array;
  origin: { toArray(): number[] };
  surface: { getAttribute(name: string): { array: Float32Array }; getIndex(): { array: Uint32Array } };
  outline: { getAttribute(name: string): { array: Float32Array } };
};
type SmokeBody = {
  ogId: string;
  record?: SmokeRecord;
  surface: { geometry: { getAttribute(name: string): { array: Float32Array } } };
  getBrep(): { revision: number };
  transform(kind: string, params: object): void;
  matrixWorld: { elements: number[] };
};
type InlineBuffers = {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  faceRanges: Uint32Array;
  outline: Float32Array;
  edgeIds: Uint32Array;
  origin: Float64Array;
  revision: number;
  bucket: number;
  triangles: number;
};
type SmokeFixture = {
  backend: string;
  threeRevision: string;
  reservedNameCollisions: string[];
  renderer: { domElement: HTMLCanvasElement };
  body: SmokeBody;
  render(): void;
  settled(): Promise<{ failed: unknown[] }>;
  exportStep(): Promise<{ text: string; report: { products: number } }>;
  resolveHit(): { ogId: string; faceId?: number } | undefined;
  getInlineBuffers(): InlineBuffers;
  workerReplies(): Promise<({ request?: number; error?: { code: string } } | undefined)[]>;
  dispose(): void;
};

async function checkSmokeScene({ expected, version }: { expected: string; version: string }): Promise<void> {
  const fixture = (window as typeof window & { __ogTest: SmokeFixture }).__ogTest;
  if (fixture.backend !== expected) throw new Error(`Expected ${expected}, got ${fixture.backend}`);
  if (fixture.threeRevision !== version) throw new Error('unexpected three revision');
  if (fixture.reservedNameCollisions.length) {
    throw new Error(`Reserved name collision: ${fixture.reservedNameCollisions.join(', ')}`);
  }
  const initial = fixture.body.getBrep().revision;
  const settled = await fixture.settled();
  if (settled.failed.length || !fixture.body.record) throw new Error('Geometry did not settle');
  const canvas = document.createElement('canvas');
  canvas.width = fixture.renderer.domElement.width;
  canvas.height = fixture.renderer.domElement.height;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('The 2D canvas context is unavailable');
  context.drawImage(fixture.renderer.domElement, 0, 0);
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
  const channel = (offset: number): number => pixels[offset] ?? Number.NaN;
  let bluePixels = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    if (channel(index + 2) > channel(index) + 30 && channel(index + 2) > channel(index + 1) + 20) bluePixels++;
  }
  if (bluePixels < 100) throw new Error(`The canvas did not draw the cube: ${String(bluePixels)} blue pixels`);
  const inline = fixture.getInlineBuffers();
  const record = fixture.body.record;
  const equalBytes = (a: ArrayBufferView, b: ArrayBufferView): boolean => {
    const left = new Uint8Array(a.buffer, a.byteOffset, a.byteLength);
    const right = new Uint8Array(b.buffer, b.byteOffset, b.byteLength);
    return left.length === right.length && left.every((value, index) => value === right[index]);
  };
  if (!equalBytes(inline.positions, record.surface.getAttribute('position').array)
    || !equalBytes(inline.normals, record.surface.getAttribute('normal').array)
    || !equalBytes(inline.indices, record.surface.getIndex().array)
    || !equalBytes(inline.faceRanges, record.faceRanges)
    || !equalBytes(inline.outline, record.outline.getAttribute('position').array)
    || !equalBytes(inline.edgeIds, record.edgeIds)
    || !equalBytes(inline.origin, new Float64Array(record.origin.toArray()))
    || inline.revision !== record.revision || inline.bucket !== record.bucket
    || inline.triangles !== record.triangles) throw new Error('Inline and worker buffers differ');
  fixture.body.transform('Translate', { offset: [2, 0, 0] });
  fixture.render();
  const translation = fixture.body.matrixWorld.elements[12] ?? Number.NaN;
  if (Math.abs(translation - 2) > 1e-9) throw new Error('Placement was not flushed');
  if (fixture.body.getBrep().revision !== initial) throw new Error('Transform changed the BRep revision');
  const hit = fixture.resolveHit();
  if (hit?.ogId !== fixture.body.ogId || hit.faceId === undefined) {
    throw new Error('Picking did not resolve a face');
  }
  const exported = await fixture.exportStep();
  if (exported.report.products !== 1 || !exported.text.includes('MANIFOLD_SOLID_BREP')) {
    throw new Error('STEP export failed');
  }
  if (expected === 'worker') {
    const replies = await fixture.workerReplies();
    const [init, nonsense, snapshot] = replies;
    if (init?.request !== 0 || !init.error || nonsense?.request !== 7 || nonsense.error?.code !== 'InvalidParameter'
      || snapshot?.request !== 8 || snapshot.error?.code !== 'InvalidParameter') {
      throw new Error(`Worker guard replies differ: ${JSON.stringify(replies)}`);
    }
  }
}

for (const backend of ['inline', 'worker'] as const) {
  test(`SDK renders, transforms, picks and exports with ${backend}`, async ({ page }) => {
    await page.goto(`/smoke.html?backend=${backend}`);
    await expect(page.locator('#status')).toHaveText('Kernel loaded');
    try {
      await page.evaluate(checkSmokeScene, {
        expected: backend, version: process.env.OG_THREE_VERSION === '184' ? '184' : '168',
      });
    } finally {
      await page.evaluate(() => { (window as typeof window & { __ogTest: SmokeFixture }).__ogTest.dispose(); });
    }
  });
}
