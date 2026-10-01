import type * as THREE from 'three';
import type { OpenGeometry, Solid, SystemAssembly } from '../../../../dist/index.js';

type Bounds = [number, number, number, number, number, number];
type Settled = ReturnType<typeof OpenGeometry.settled>;
type StepExport = Awaited<ReturnType<typeof OpenGeometry.exportStep>>;
type PickResult = ReturnType<typeof OpenGeometry.resolveHit>;

export type InlineBuffers = {
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

export type SmokeFixture = {
  body: Solid;
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  threeRevision: string;
  reservedNameCollisions: string[];
  reservedNamesDerived: number;
  readonly backend: string;
  render(): void;
  settled(): Settled;
  exportStep(): Promise<StepExport>;
  resolveHit(): PickResult;
  resolveSceneHit(): { hit: PickResult; outlineHit: boolean };
  getInlineBuffers(): InlineBuffers;
  workerReplies(): Promise<unknown[]>;
  dispose(): void;
};

export type AcceptanceResult = {
  baseVolume: number;
  cutVolume: number;
  railVolume: number;
  rebuiltVolume: number;
  recutVolume: number;
  railBounds: Bounds;
  wallBounds: Bounds;
  coverageGap: boolean;
  products: number;
  skipped: number;
  pcurvelessEdges: number;
  stepBytes: number;
};

export type StoreyResult = {
  walls: number;
  openingsPerWall: number;
  instances: number;
  flushCount: number;
  renderFlushes: number;
  geometryCount: number;
  renderMs: number;
};

export type StoreyPerformance = {
  transformFlushMs: number;
  transformMs: number;
  flushMs: number;
  stepMs: number;
  bytes: number;
  products: number;
  entities: number;
};

export type InstanceMemory = {
  one: number;
  shared: number;
  unique: number;
  after: number;
  sharedDetails: { target: string | undefined; copy: string | undefined; same: boolean };
};

export type MemoryResult = InstanceMemory & {
  baseline: number;
  outlineOne: number;
  outlineShared: number;
  outlineUnique: number;
  outlineAfter: number;
};

export type CrashResult = {
  afterRestart: string;
  afterFallback: string;
  sameRecord: boolean;
  errors: string[];
  drawnInSameRender: boolean;
};

export type FailureResult = { failed: number; hasRecord: boolean; errors: string[]; backend: string };

export type PixelResult = {
  before: number[];
  after: number[];
  matrixMatches: boolean;
  geometryMatches: boolean;
  syncHit: boolean;
};

export type StaleResult = { current: number; displayed: number | undefined; jobs: number };

export type TransactionResult = {
  threw: boolean;
  revived: boolean;
  dryResult: string;
  dryRestored: boolean;
  thenableCode: string;
};

export type ReuseResult = { revived: boolean; gone: boolean; errors: unknown[] };

export type RetryResult = {
  warning: { code: string; bucket: number; retryBucket: number } | undefined;
  bucket: number | undefined;
  errors: unknown[];
};

export type OrbitResult = { jobs: number; buckets: number[]; sendBuckets: number[] };

export type AcceptanceFixture = {
  bodies: Solid[];
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  level: SystemAssembly;
  wall: Solid;
  rail: Solid;
  rails: Solid[];
  result: AcceptanceResult;
  readonly backend: string;
  flush(): void;
  settled(): Settled;
  render(): void;
  exportStep(): Promise<StepExport>;
  buildStorey(): Promise<StoreyResult>;
  storeyPerformanceProbe(): Promise<StoreyPerformance>;
  memoryProbe(): Promise<MemoryResult>;
  lodProbe(): Promise<{ before: number; after: number }>;
  workerCrashProbe(): Promise<CrashResult>;
  workerFailureProbe(): Promise<FailureResult>;
  placementPixelProbe(): PixelResult;
  onDemandProbe(): Promise<{ events: number; appeared: boolean }>;
  staleWorkerProbe(): Promise<StaleResult>;
  snapshotResendProbe(): Promise<{ triangles: number }>;
  lodHysteresisProbe(): Promise<{ p: number; buckets: number[] }>;
  orbitProbe(): Promise<OrbitResult>;
  coarserRetryProbe(): Promise<RetryResult>;
  transactionProbe(): TransactionResult;
  reuseProbe(): Promise<ReuseResult>;
  dispose(): void;
};

declare global {
  var ogSmoke: SmokeFixture | undefined;
  var ogAcceptance: AcceptanceFixture | undefined;
}
