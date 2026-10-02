import assert from 'node:assert/strict';
import { test } from 'node:test';
import { selectSteps, wasmFloorFindings } from './verify.mjs';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const PADDING = ' '.repeat(50);
const NO_TESTS = 'no tests to run!';
const WASM_CORE = 'tests/wasm_core.rs';
const PASSED = [
  `Executing bindgen...${PADDING}\r${PADDING}\rrunning 4 tests`,
  'test embedded_brep_parity_and_snapshot_buffers ... ok',
  '',
  'test result: ok. 4 passed; 0 failed; 0 ignored; 0 filtered out; finished in 1.22s',
  '',
];
const BINARIES = [
  'unittests src/lib.rs', 'tests/booleans/main.rs', 'tests/errors.rs', WASM_CORE, 'tests/world_graph/main.rs',
];

function wasmLog(outputs) {
  const entries = BINARIES.map((binary, index) => [
    `${index === 0 ? '' : `${PADDING}\r`}     Running ${binary} (target/wasm32-unknown-unknown/debug/deps/x.wasm)`,
    ...(outputs.get(binary) ?? [NO_TESTS]),
  ]);
  return [
    '$ wasm-pack test --node',
    '    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.03s',
    ...entries.flat(),
    `${PADDING}\r[WARN]: There's a newer version of wasm-pack available`,
  ].join('\n');
}

function names(argv) {
  return selectSteps(argv).map((step) => step.name);
}

function failsWith(message) {
  return (error) => error instanceof Error && error.message === message;
}

test('--only keeps the named steps in check order', () => {
  assert.deepEqual(names(['--only', 'typecheck,build']), ['build', 'typecheck']);
});

test('the wasm test step is named test:wasm like its npm script', () => {
  assert.deepEqual(names(['--only', 'test:wasm']), ['test:wasm']);
});

test('every cargo gate step runs in a directory that holds a Cargo.toml', () => {
  const cargoSteps = selectSteps(['--full']).filter((step) => step.commands?.some((command) => command[0] === 'cargo'));
  assert(cargoSteps.length > 0);
  const missing = cargoSteps
    .filter((step) => !existsSync(path.join(REPOSITORY_ROOT, step.cwd ?? '', 'Cargo.toml')))
    .map((step) => step.name);
  assert.deepEqual(missing, []);
});

test('--from starts at the named step', () => {
  assert.deepEqual(names(['--from', 'test:node']), ['test:node', 'test:performance']);
});

test('--skip drops the named steps', () => {
  assert.deepEqual(names(['--from', 'test:bindings', '--skip', 'test:node']), ['test:bindings', 'test:performance']);
});

test('--full adds both browser steps', () => {
  assert.deepEqual(names(['--full', '--from', 'test:performance']), [
    'test:performance', 'browser:three-168', 'browser:three-184',
  ]);
});

test('--only names before --from select nothing and fail', () => {
  assert.throws(() => selectSteps(['--from', 'test:node', '--only', 'build']), failsWith('nothing selected'));
});

test('--skip of every selected step selects nothing and fails', () => {
  assert.throws(() => selectSteps(['--only', 'build', '--skip', 'build']), failsWith('nothing selected'));
});

test('an unknown step name fails with one line naming it', () => {
  assert.throws(
    () => selectSteps(['--skip', 'nope']),
    (error) => error instanceof Error && error.message.startsWith('unknown step nope;')
      && !error.message.includes('\n'),
  );
});

test('an option without a value fails naming the option', () => {
  assert.throws(() => selectSteps(['--only']), failsWith('--only needs a value'));
  assert.throws(() => selectSteps(['--from', '--full']), failsWith('--from needs a value'));
});

test('a wasm log where every binary runs no tests fails the wasm floor', () => {
  assert.deepEqual(wasmFloorFindings(wasmLog(new Map())), [
    'no wasm test binary reported a passing test',
    `${WASM_CORE} ran no tests`,
  ]);
});

test('a wasm log whose wasm_core binary runs no tests fails the wasm floor', () => {
  const log = wasmLog(new Map([['tests/errors.rs', PASSED]]));
  assert.deepEqual(wasmFloorFindings(log), [`${WASM_CORE} ran no tests`]);
});

test('the recorded wasm log shape passes the wasm floor', () => {
  assert.deepEqual(wasmFloorFindings(wasmLog(new Map([[WASM_CORE, PASSED]]))), []);
});

test('the wasm test step carries both its command and the floor', () => {
  const [step] = selectSteps(['--only', 'test:wasm']);
  assert.deepEqual(step?.commands, [['wasm-pack', 'test', '--node']]);
  assert.equal(step?.logCheck, wasmFloorFindings);
});
