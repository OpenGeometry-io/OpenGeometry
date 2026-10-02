import assert from 'node:assert/strict';
import { test } from 'node:test';
import { distManifest, packageReadme } from './dist-package.mjs';

const REPOSITORY = {
  type: 'git', url: 'git+https://github.com/OpenGeometry-io/OpenGeometry.git', directory: 'main',
};
const BUGS = { url: 'https://github.com/OpenGeometry-io/OpenGeometry/issues' };
const SOURCE_MANIFEST = {
  name: 'opengeometry',
  version: '2.5.0',
  private: true,
  type: 'module',
  license: 'MPL-2.0',
  description: 'A CAD kernel.',
  keywords: ['cad', 'brep'],
  homepage: 'https://opengeometry.io?utm_source=npm',
  repository: REPOSITORY,
  bugs: BUGS,
  peerDependencies: { three: '>=0.168.0 <0.185.0' },
  scripts: { build: 'rollup -c' },
  devDependencies: { rollup: '4.36.0' },
};
const BLOB = 'https://github.com/OpenGeometry-io/OpenGeometry/blob/main';
const TREE = 'https://github.com/OpenGeometry-io/OpenGeometry/tree/main';
const RAW = 'https://raw.githubusercontent.com/OpenGeometry-io/OpenGeometry/main';

test('the dist manifest carries the package details and the entry points', () => {
  const manifest = distManifest(SOURCE_MANIFEST);
  assert.deepEqual(manifest, {
    name: 'opengeometry',
    version: '2.5.0',
    description: 'A CAD kernel.',
    keywords: ['cad', 'brep'],
    homepage: 'https://opengeometry.io?utm_source=npm',
    repository: REPOSITORY,
    bugs: BUGS,
    license: 'MPL-2.0',
    type: 'module',
    main: './index.js',
    types: './index.d.ts',
    exports: {
      '.': { types: './index.d.ts', import: './index.js' },
      './tessellation-worker.js': './tessellation-worker.js',
      './opengeometry_bg.wasm': './opengeometry_bg.wasm',
      './package.json': './package.json',
    },
    peerDependencies: { three: '>=0.168.0 <0.185.0' },
  });
  assert.deepEqual(Object.keys(manifest), [
    'name', 'version', 'description', 'keywords', 'homepage', 'repository', 'bugs', 'license', 'type', 'main',
    'types', 'exports', 'peerDependencies',
  ]);
});

test('the package readme tags its links for npm', () => {
  const source = [
    '[Website](https://opengeometry.io?utm_source=github)',
    '<a href="https://demos.opengeometry.io?utm_source=github">Demos</a>',
    '[Guide](https://docs.opengeometry.io/OpenGeometry?utm_source=github#start)',
  ].join('\n');
  assert.equal(packageReadme(source), [
    '[Website](https://opengeometry.io?utm_source=npm)',
    '<a href="https://demos.opengeometry.io?utm_source=npm">Demos</a>',
    '[Guide](https://docs.opengeometry.io/OpenGeometry?utm_source=npm#start)',
  ].join('\n'));
});

test('the package readme makes relative links absolute and leaves absolute links and anchors alone', () => {
  const source = [
    'See the [licence](./LICENSE.md) and the [examples](./main/opengeometry-three/examples-vite/).',
    'Read [verification](./developer.md#verification-and-ci) or jump to [features](#features).',
    'Write to [us](mailto:someone@example.com) or open <a href="./LICENSE.md">the licence</a>.',
    '![x](./docs/x.png) and [`release.md`](release.md "Releases")',
    '```md',
    '[inside](./LICENSE.md)',
    '```',
    'Inline `[code](./LICENSE.md)` stays.',
  ].join('\n');
  assert.equal(packageReadme(source), [
    `See the [licence](${BLOB}/LICENSE.md) and the [examples](${TREE}/main/opengeometry-three/examples-vite/).`,
    `Read [verification](${BLOB}/developer.md#verification-and-ci) or jump to [features](#features).`,
    `Write to [us](mailto:someone@example.com) or open <a href="${BLOB}/LICENSE.md">the licence</a>.`,
    `![x](${RAW}/docs/x.png) and [\`release.md\`](${BLOB}/release.md "Releases")`,
    '```md',
    '[inside](./LICENSE.md)',
    '```',
    'Inline `[code](./LICENSE.md)` stays.',
  ].join('\n'));
  assert.throws(() => packageReadme('[up](../README.md)'));
});
