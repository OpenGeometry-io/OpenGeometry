import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as TESTING from '../../../dist/testing.js';

const MODULE = new WebAssembly.Module(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
const JOB = { shapeId: 'shape-1', revision: 2, bucket: 0.01, priority: 0, maxTriangles: 1000, generation: 3 };

function parse(message) {
  return Reflect.get(TESTING, 'parseWorkerMessage')(message);
}

test('worker messages with unknown kinds, extra keys or wrong field types are rejected with InvalidParameter', () => {
  const rows = [
    { message: { kind: 'nonsense', request: 7 }, key: 'kind', request: 7 },
    { message: { kind: 'init', request: 0, module: 'invalid' }, key: 'module', request: 0 },
    { message: { kind: 'snapshot', request: 2, shapeId: 'shape-1', revision: 2 }, key: 'bytes', request: 2 },
    { message: { kind: 'tessellate', request: 4, ...JOB, extra: 1 }, key: 'extra', request: 4 },
    { message: { kind: 'cancel', shapeId: 'shape-1', generation: Infinity }, key: 'generation', request: undefined },
  ];
  for (const row of rows) {
    const failure = parse(row.message);
    assert.equal(failure.error.code, 'InvalidParameter', row.key);
    assert(failure.error.message.includes(row.key), failure.error.message);
    assert(Object.hasOwn(failure, 'request'), row.key);
    assert.equal(failure.request, row.request, row.key);
  }
});

test('well-formed worker messages pass the guard unchanged', () => {
  const messages = [
    { kind: 'init', request: 0, module: MODULE },
    { kind: 'snapshot', request: 1, shapeId: 'shape-1', revision: 2, bytes: new Uint8Array([1, 2, 3]) },
    { kind: 'tessellate', request: 2, ...JOB },
    { kind: 'cancel', shapeId: 'shape-1', generation: 3 },
    { kind: 'drop', shapeId: 'shape-1', revision: 2 },
  ];
  for (const message of messages) assert.equal(parse(message), message, message.kind);
});
