import assert from 'node:assert/strict';
import { test } from 'node:test';
import { classifyFlake, playwrightCounts, selectSteps } from './verify.mjs';

const MEMORY_TEST = 'instance geometry memory returns to baseline after disposal';
const SEPARATOR = '─'.repeat(12);

function names(argv) {
  return selectSteps(argv).map((step) => step.name);
}

function failsWith(message) {
  return (error) => error instanceof Error && error.message === message;
}

function browserLog(failed, passed) {
  return [
    `Running ${String(failed.length + passed)} tests using 1 worker`,
    `  1) acceptance.spec.ts:194:5 › ${failed[0] ?? ''} ${SEPARATOR}`,
    '    Error: Geometry memory changed unexpectedly',
    `  ${String(failed.length)} failed`,
    ...failed.map((title, index) => `    acceptance.spec.ts:${String(194 + index)}:5 › ${title} ${SEPARATOR}`),
    `  ${String(passed)} passed (1.4m)`,
  ].join('\n');
}

test('--only keeps the named steps in check order', () => {
  assert.deepEqual(names(['--only', 'typecheck,build']), ['build', 'typecheck']);
});

test('the wasm test step is named test:wasm like its npm script', () => {
  assert.deepEqual(names(['--only', 'test:wasm']), ['test:wasm']);
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

test('a performance regression is not classified as a flake', () => {
  const log = [
    '> opengeometry@2.5.0 test:performance',
    'Error: rotated20BooleanMs regressed: 150 ms exceeds 142 ms',
    '    at file:///repo/scripts/bench/performance.mjs:60:11',
  ].join('\n');
  assert.equal(classifyFlake('test:performance', log), undefined);
});

test('a browser run whose only failure is the memory test is a known flake', () => {
  assert.deepEqual(classifyFlake('browser:three-184', browserLog([MEMORY_TEST], 39)), {
    verdict: 'KNOWN-FLAKE', detail: MEMORY_TEST,
  });
});

test('a browser run with another failure beside the memory test is not classified', () => {
  assert.equal(classifyFlake('browser:three-168', browserLog([MEMORY_TEST, 'zoom selects a finer LOD bucket'], 38)),
    undefined);
});

test('a browser run failing only another test is not classified', () => {
  assert.equal(classifyFlake('browser:three-168', browserLog(['zoom selects a finer LOD bucket'], 39)), undefined);
});

test('a failure in any other step is not classified', () => {
  assert.equal(classifyFlake('typecheck', browserLog([MEMORY_TEST], 39)), undefined);
});

test('playwright counts read the passed and failed totals', () => {
  assert.deepEqual(playwrightCounts(browserLog([MEMORY_TEST], 4)), { passed: 4, failed: 1 });
  assert.deepEqual(playwrightCounts('  5 passed (20.1s)'), { passed: 5, failed: 0 });
});
