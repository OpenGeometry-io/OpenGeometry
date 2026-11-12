import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import frontMatter from 'front-matter';

const DOCS = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SDK = path.resolve(DOCS, '../main/opengeometry-three');
const SITE = 'https://docs.opengeometry.io';
const CHECK = process.argv.includes('--check');

const PAGES = {
  OpenGeometry: 'opengeometry/api/opengeometry',
  OGMark: 'opengeometry/api/og-mark',
  HostGroup: 'opengeometry/api/display',
  SystemAssembly: 'opengeometry/api/system-assembly',
  Wire: 'opengeometry/api/wire',
  Solid: 'opengeometry/api/solid',
  Sheet: 'opengeometry/api/sheet',
  BodyEventMap: 'opengeometry/api/body',
  PointClassification: 'opengeometry/api/body',
  Measure: 'opengeometry/api/body',
  Validity: 'opengeometry/api/body',
  MirrorPlane: 'opengeometry/api/body',
  OGError: 'opengeometry/api/og-error',
  ErrorDetails: 'opengeometry/api/og-error',
  OperateErrorDetails: 'opengeometry/api/og-error',
  AnalyticExchangeBodyV2: 'opengeometry/api/export',
  IfcPreparedValue: 'opengeometry/api/export',
  OffsetRegion: 'opengeometry/api/regions',
  CurvedRegion2D: 'opengeometry/api/regions',
  CurvedRegionEdge: 'opengeometry/api/regions',
  OffsetHole: 'opengeometry/api/offsets',
  OffsetPath2D: 'opengeometry/api/offsets',
  OffsetPolylineOptions: 'opengeometry/api/offsets',
};
const CONSTANTS_PAGE = 'opengeometry/api/constants';
const PARAMS_TYPE = /^(Profile\w*|\w+Params)$/;

function names(list) {
  return list
    .split(',')
    .map((name) => name.trim().replace(/^type\s+/, ''))
    .filter(Boolean);
}

function moduleExports(file, kind = 'value') {
  const text = readFileSync(file, 'utf8');
  const found = [];
  for (const [, typeOnly, list] of text.matchAll(/export\s+(type\s+)?\{([^}]*)\}\s+from/g)) {
    for (const name of names(list)) found.push({ export: name, kind: typeOnly ? 'type' : kind });
  }
  for (const [, from] of text.matchAll(/export\s+\*\s+from\s+'([^']+)'/g)) {
    const target = path.resolve(path.dirname(file), from);
    found.push(...moduleExports(target.endsWith('.ts') ? target : `${target}.ts`, kind));
  }
  for (const [, name] of text.matchAll(/export\s+const\s+(\w+)/g))
    found.push({ export: name, kind: 'value' });
  return found;
}

function navigationPages(node, trail = []) {
  if (typeof node === 'string') return [{ page: node, trail }];
  if (Array.isArray(node)) return node.flatMap((item) => navigationPages(item, trail));
  const label = node.tab ?? node.group;
  const next = label ? [...trail, label] : trail;
  return [node.pages, node.groups, node.tabs]
    .filter(Boolean)
    .flatMap((child) => navigationPages(child, next));
}

function pageText(page) {
  return readFileSync(path.join(DOCS, `${page}.mdx`), 'utf8');
}

const problems = [];
const coverage = moduleExports(path.join(SDK, 'index.ts')).map((entry) => {
  const constant = entry.export.startsWith('OG_') || PARAMS_TYPE.test(entry.export);
  const page = PAGES[entry.export] ?? (constant ? CONSTANTS_PAGE : undefined);
  if (!page) problems.push(`${entry.export} has no API page`);
  else if (!new RegExp(`\\b${entry.export}\\b`).test(pageText(page)))
    problems.push(`${entry.export} is not named on ${page}`);
  return { ...entry, page };
});
for (const name of Object.keys(PAGES)) {
  if (!coverage.some((entry) => entry.export === name))
    problems.push(`${name} is mapped but not exported`);
}

const docs = JSON.parse(readFileSync(path.join(DOCS, 'docs.json'), 'utf8'));
const pages = navigationPages(docs.navigation.tabs);
const sections = new Map();
for (const { page, trail } of pages) {
  const { attributes } = frontMatter(pageText(page));
  if (!attributes.title || !attributes.description)
    problems.push(`${page} needs a title and a description`);
  const section = trail.filter((label) => label !== 'Documentation').join(' / ') || 'Documentation';
  if (!sections.has(section)) sections.set(section, []);
  sections
    .get(section)
    .push(`- [${attributes.title}](${SITE}/${page}.md): ${attributes.description}`);
}

const llms = [
  `# ${docs.name}`,
  '',
  `> ${docs.description}`,
  '',
  'Package: `opengeometry` on npm. These docs describe version 3.0. Check `node_modules/opengeometry/package.json` and `index.d.ts` before you use an example.',
  '',
  'Points and vectors are `[x, y, z]` arrays, units are metres and Y is up. Await `OpenGeometry.create` once before the first body. Code that uses `Cuboid`, `Vector3` or `setPlacement` is written for 2.0 and does not work with 3.0.',
  '',
  ...[...sections].flatMap(([section, lines]) => [`## ${section}`, '', ...lines, '']),
].join('\n');

const outputs = {
  'api-coverage.json': `${JSON.stringify(coverage, null, 2)}\n`,
  'llms.txt': llms,
};

for (const [name, text] of Object.entries(outputs)) {
  const file = path.join(DOCS, name);
  if (!CHECK) writeFileSync(file, text);
  else if (readFileSync(file, 'utf8') !== text)
    problems.push(`${name} is stale: run npm run index`);
}

if (problems.length > 0) {
  console.error(problems.join('\n'));
  process.exitCode = 1;
}
