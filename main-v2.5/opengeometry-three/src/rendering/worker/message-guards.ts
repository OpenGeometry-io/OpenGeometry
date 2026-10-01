import type { WorkerMessage } from '../worker-protocol.js';

export type GuardFailure = { request: number | undefined; error: { code: 'InvalidParameter'; message: string } };

type Check = (value: unknown) => boolean;

const KINDS = ['init', 'snapshot', 'tessellate', 'cancel', 'drop'] as const;

export function parseWorkerMessage(data: unknown): WorkerMessage | GuardFailure {
  if (isWorkerMessage(data)) return data;
  const message = problemOf(data) ?? 'invalid worker message';
  return { request: requestOf(data), error: { code: 'InvalidParameter', message } };
}

function isWorkerMessage(data: unknown): data is WorkerMessage {
  return problemOf(data) === undefined;
}

function problemOf(data: unknown): string | undefined {
  if (typeof data !== 'object' || data === null) return 'worker message is not an object';
  const kind: unknown = Reflect.get(data, 'kind');
  const known = KINDS.find((name) => name === kind);
  if (known === undefined) return `worker message field kind has unknown value ${String(kind)}`;
  const checks = fieldChecks(known);
  const extra = Object.keys(data).find((key) => !Object.hasOwn(checks, key));
  if (extra !== undefined) return `unknown worker message field ${extra}`;
  const invalid = Object.entries(checks).find(([key, check]) => !check(Reflect.get(data, key)));
  return invalid === undefined ? undefined : `worker message field ${invalid[0]} is missing or invalid`;
}

function fieldChecks(kind: (typeof KINDS)[number]): Record<string, Check> {
  switch (kind) {
    case 'init':
      return { kind: isKind, request: (value) => value === 0, module: (value) => value instanceof WebAssembly.Module };
    case 'snapshot':
      return {
        kind: isKind, request: isFiniteNumber, shapeId: isShapeId, revision: isFiniteNumber,
        bytes: (value) => value instanceof Uint8Array,
      };
    case 'tessellate':
      return {
        kind: isKind, request: isFiniteNumber, shapeId: isShapeId, revision: isFiniteNumber, bucket: isFiniteNumber,
        priority: isFiniteNumber, maxTriangles: isFiniteNumber, generation: isFiniteNumber,
      };
    case 'cancel':
      return { kind: isKind, shapeId: isShapeId, generation: isFiniteNumber };
    case 'drop':
      return { kind: isKind, shapeId: isShapeId, revision: isFiniteNumber };
  }
}

function requestOf(data: unknown): number | undefined {
  const request: unknown = typeof data === 'object' && data !== null ? Reflect.get(data, 'request') : undefined;
  return isFiniteNumber(request) ? request : undefined;
}

function isKind(): boolean {
  return true;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isShapeId(value: unknown): boolean {
  return typeof value === 'string' && value.length > 0;
}
