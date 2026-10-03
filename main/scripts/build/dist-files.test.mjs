import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import path from 'node:path';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const DIST = path.join(REPOSITORY_ROOT, 'dist');
const LICENCE = path.join(REPOSITORY_ROOT, '..', 'LICENSE.md');
const FENCE = /^```[^\n]*\n[\s\S]*?^```$/gm;
const TARGET = /\]\(([^()\s]+)|^\s{0,3}\[[^\]]+\]:\s*(\S+)|\b(?:href|src)="([^"]*)"/gm;
const ABSOLUTE = /^(?:[a-z][a-z0-9+.-]*:|#|\/\/)/i;

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
