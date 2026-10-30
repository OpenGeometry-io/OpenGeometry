import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { test } from 'node:test';
import path from 'node:path';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const DIST = path.join(REPOSITORY_ROOT, 'dist');
const LICENCE = path.join(REPOSITORY_ROOT, '..', 'LICENSE.md');
const FENCE = /^```[^\n]*\n[\s\S]*?^```$/gm;
const TARGET = /\]\(([^()\s]+)|^\s{0,3}\[[^\]]+\]:\s*(\S+)|\b(?:href|src)="([^"]*)"/gm;
const ABSOLUTE = /^(?:[a-z][a-z0-9+.-]*:|#|\/\/)/i;
const ENTRIES = ['index.js', 'testing.js', 'tessellation-worker.js'];
const LOCAL_IMPORT = /(?:\bfrom\s*|\bimport\(\s*)'\.\/([^'/]+\.js)'/g;

function importedChunks(file) {
  return [...readFileSync(path.join(DIST, file), 'utf8').matchAll(LOCAL_IMPORT)]
    .flatMap((match) => match[1] ?? []);
}

function reachableScripts() {
  const reached = new Set(ENTRIES);
  const pending = [...ENTRIES];
  for (let file = pending.pop(); file !== undefined; file = pending.pop()) {
    for (const chunk of importedChunks(file)) {
      if (!reached.has(chunk)) {
        reached.add(chunk);
        pending.push(chunk);
      }
    }
  }
  return reached;
}

test('the built package ships its readme and licence', () => {
  const readmeFile = path.join(DIST, 'README.md');
  const licenceFile = path.join(DIST, 'LICENSE.md');
  assert(existsSync(readmeFile), `${readmeFile} is missing`);
  assert(existsSync(licenceFile), `${licenceFile} is missing`);
  const readme = readFileSync(readmeFile, 'utf8');
  assert.equal(readme.includes('utm_source=github'), false);
  const targets = [...readme.replace(FENCE, '').matchAll(TARGET)]
    .map((match) => match[1] ?? match[2] ?? match[3] ?? '');
  assert(targets.length > 0, 'the readme has no link');
  assert.deepEqual(targets.filter((target) => !ABSOLUTE.test(target)), []);
  assert.equal(readFileSync(licenceFile, 'utf8'), readFileSync(LICENCE, 'utf8'));
});

test('the built package holds no script the current build does not load', () => {
  const scripts = readdirSync(DIST, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.js'))
    .map((entry) => entry.name);
  const reachable = reachableScripts();
  assert.deepEqual(scripts.filter((file) => !reachable.has(file)), []);
});
