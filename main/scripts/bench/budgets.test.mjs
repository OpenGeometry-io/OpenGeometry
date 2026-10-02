import assert from 'node:assert/strict';
import { test } from 'node:test';
import { budgetFindings, median } from './budgets.mjs';

const BASELINE = {
  sizes: { wall50Triangles: 1000 },
  timings: { 'darwin-arm64': { measuredOn: '2026-10-01', wall50BooleanMs: 100 } },
};
const KEYS = [['darwin-arm64', false], ['linux-x64', false], ['darwin-arm64', true], ['linux-x64', true]];

function sized(sizes) {
  return { sizes, timings: {} };
}

function timed(timings) {
  return { sizes: { wall50Triangles: 1000 }, timings };
}

test('a size more than 25 percent over its baseline fails on every platform', () => {
  for (const [key, ci] of KEYS) {
    assert.deepEqual(budgetFindings(sized({ wall50Triangles: 1250 }), BASELINE, key, ci).findings, []);
    assert.deepEqual(budgetFindings(sized({ wall50Triangles: 1251 }), BASELINE, key, ci).findings, [
      'wall50Triangles regressed: 1251 exceeds 1250',
    ]);
  }
});

test('a measured size with no baseline key fails on every platform', () => {
  for (const [key, ci] of KEYS) {
    assert.deepEqual(budgetFindings(sized({ mitred20Triangles: 10 }), BASELINE, key, ci).findings, [
      'missing size baseline for mitred20Triangles',
    ]);
  }
});

test('a timing more than 25 percent over its baseline fails on the baseline platform', () => {
  assert.deepEqual(budgetFindings(timed({ wall50BooleanMs: 125 }), BASELINE, 'darwin-arm64', false), {
    findings: [], notes: [],
  });
  assert.deepEqual(budgetFindings(timed({ wall50BooleanMs: 126 }), BASELINE, 'darwin-arm64', false).findings, [
    'wall50BooleanMs regressed: 126 ms exceeds 125 ms',
  ]);
});

test('no timing is checked when CI is true', () => {
  for (const key of ['darwin-arm64', 'linux-x64']) {
    assert.deepEqual(budgetFindings(timed({ wall50BooleanMs: 1000 }), BASELINE, key, true), {
      findings: [], notes: ['timings are not checked on CI'],
    });
  }
});

test('a missing timing block is only noted when CI is unset', () => {
  assert.deepEqual(budgetFindings(timed({ wall50BooleanMs: 1 }), BASELINE, 'linux-x64', false), {
    findings: [], notes: ['missing timing baseline for linux-x64'],
  });
});

test('a timing key missing from an existing block is only noted when CI is unset', () => {
  assert.deepEqual(budgetFindings(timed({ mitred20BooleanMs: 1 }), BASELINE, 'darwin-arm64', false), {
    findings: [], notes: ['missing timing baseline for mitred20BooleanMs on darwin-arm64'],
  });
});

test('the median of five ignores one slow run', () => {
  assert.equal(median([10, 11, 500, 12, 9]), 11);
});
