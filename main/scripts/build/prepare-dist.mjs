import { cp, mkdir, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';
import { withJsSpecifiers } from './declaration-specifiers.mjs';
import { distManifest, packageReadme } from './dist-package.mjs';

const DIST = path.join(REPOSITORY_ROOT, 'dist');
const PKG = path.join(REPOSITORY_ROOT, 'opengeometry', 'pkg');
const DIST_PKG = path.join(DIST, 'opengeometry', 'pkg');
const PUBLIC_ROOT = path.join(REPOSITORY_ROOT, '..');

async function declarationFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory)) {
    const candidate = path.join(directory, entry);
    if ((await stat(candidate)).isDirectory()) {
      files.push(...await declarationFiles(candidate));
    } else if (candidate.endsWith('.d.ts')) {
      files.push(candidate);
    }
  }
  return files;
}

await mkdir(DIST_PKG, { recursive: true });
for (const file of ['opengeometry_bg.wasm', 'opengeometry_bg.wasm.d.ts']) {
  await cp(path.join(PKG, file), path.join(DIST, file));
}
for (const file of ['opengeometry.js', 'opengeometry.d.ts', 'opengeometry_bg.wasm', 'opengeometry_bg.wasm.d.ts']) {
  await cp(path.join(PKG, file), path.join(DIST_PKG, file));
}

await writeFile(path.join(DIST, 'testing.d.ts'), "export * from './src/testing/internals.js';\n");

for (const file of await declarationFiles(DIST)) {
  const source = await readFile(file, 'utf8');
  const target = path.relative(path.dirname(file), path.join(DIST_PKG, 'opengeometry.js')).split(path.sep).join('/');
  const specifier = target.startsWith('.') ? target : `./${target}`;
  const rewritten = withJsSpecifiers(source.replace(/(?:\.\.\/)+opengeometry\/pkg\/opengeometry(?:\.js)?/g, specifier));
  if (rewritten !== source) await writeFile(file, rewritten);
}

const MANIFEST = JSON.parse(await readFile(path.join(REPOSITORY_ROOT, 'package.json'), 'utf8'));
await writeFile(path.join(DIST, 'package.json'), `${JSON.stringify(distManifest(MANIFEST), null, 2)}\n`);
const README = await readFile(path.join(PUBLIC_ROOT, 'README.md'), 'utf8');
await writeFile(path.join(DIST, 'README.md'), packageReadme(README));
await cp(path.join(PUBLIC_ROOT, 'LICENSE.md'), path.join(DIST, 'LICENSE.md'));
