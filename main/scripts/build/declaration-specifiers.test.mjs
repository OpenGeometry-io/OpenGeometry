import assert from 'node:assert/strict';
import { test } from 'node:test';
import { withJsSpecifiers } from './declaration-specifiers.mjs';

test('a relative specifier gains .js', () => {
  const source = [
    "export { OpenGeometry } from './src/opengeometry';",
    "export * from './src/constants/index';",
    "import type { Body } from '../bodies/body';",
    'import { Vector } from "./vector";',
  ].join('\n');
  assert.equal(withJsSpecifiers(source), [
    "export { OpenGeometry } from './src/opengeometry.js';",
    "export * from './src/constants/index.js';",
    "import type { Body } from '../bodies/body.js';",
    'import { Vector } from "./vector.js";',
  ].join('\n'));
});

test('a specifier with an extension or a package name is left alone', () => {
  const source = [
    "export * from './src/testing/internals.js';",
    "import type { InitInput } from '../../opengeometry/pkg/opengeometry.js';",
    "import * as THREE from 'three';",
    "import type { Line2 } from 'three/examples/jsm/lines/Line2';",
  ].join('\n');
  assert.equal(withJsSpecifiers(source), source);
});
