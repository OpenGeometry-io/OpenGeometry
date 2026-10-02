import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import path from 'node:path';
import ts from 'typescript';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const README = {
  label: 'the README', file: path.join(REPOSITORY_ROOT, '..', 'README.md'), prefix: 'readme-block',
};
const MIGRATION = {
  label: 'MIGRATION.md', file: path.join(REPOSITORY_ROOT, '..', 'MIGRATION.md'), prefix: 'migration-block',
};
const TS_BLOCK = /^```ts\n([\s\S]*?)^```$/gm;
const TS_OPENER = /^[ \t]*(?:`{3,}|~{3,})[ \t]*(?:typescript|tsx|ts)(?![\w-])/gim;
const QUICK_START_PAGE = path.join(
  REPOSITORY_ROOT, 'opengeometry-three', 'tests', 'browser', 'pages', 'quick-start.ts',
);
const OPTIONS = {
  noEmit: true,
  strict: true,
  skipLibCheck: true,
  types: [],
  target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  lib: ['lib.es2022.d.ts', 'lib.dom.d.ts'],
  paths: { opengeometry: [path.join(REPOSITORY_ROOT, 'dist', 'index.d.ts')] },
};

function documentText(source) {
  assert(existsSync(source.file), `${source.label} does not exist at ${source.file}`);
  return readFileSync(source.file, 'utf8');
}

function tsBlocks(source) {
  const blocks = new Map();
  for (const match of documentText(source).matchAll(TS_BLOCK)) {
    blocks.set(path.join(REPOSITORY_ROOT, `${source.prefix}-${String(blocks.size + 1)}.ts`), match[1]);
  }
  return blocks;
}

function blockHost(blocks) {
  const host = ts.createCompilerHost(OPTIONS);
  return {
    ...host,
    fileExists: (name) => blocks.has(name) || host.fileExists(name),
    readFile: (name) => blocks.get(name) ?? host.readFile(name),
    getSourceFile: (name, language, onError, shouldCreate) => {
      const text = blocks.get(name);
      if (text === undefined) return host.getSourceFile(name, language, onError, shouldCreate);
      return ts.createSourceFile(name, text, language, true);
    },
  };
}

function diagnosticText(diagnostic) {
  const { file, start } = diagnostic;
  const where = file === undefined || start === undefined
    ? ''
    : `${path.basename(file.fileName)} line ${String(file.getLineAndCharacterOfPosition(start).line + 1)} `;
  return `${where}TS${String(diagnostic.code)}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')}`;
}

function blockDiagnostics(program, names) {
  return names.flatMap((name) => {
    const file = program.getSourceFile(name);
    assert(file !== undefined, `${name} was not loaded`);
    return [...program.getSyntacticDiagnostics(file), ...program.getSemanticDiagnostics(file)];
  });
}

for (const source of [README, MIGRATION]) {
  test(`every ts block in ${source.label} type-checks against the built package`, () => {
    const blocks = tsBlocks(source);
    assert(blocks.size > 0, `${source.label} has no ts block`);
    const program = ts.createProgram([...blocks.keys()], OPTIONS, blockHost(blocks));
    const diagnostics = [
      ...program.getOptionsDiagnostics(),
      ...program.getGlobalDiagnostics(),
      ...blockDiagnostics(program, [...blocks.keys()]),
    ];
    assert.deepEqual(diagnostics.map(diagnosticText), []);
  });

  test(`every ts fence in ${source.label} is one the type check reads`, () => {
    const openers = [...documentText(source).matchAll(TS_OPENER)].length;
    assert.equal(openers, tsBlocks(source).size);
  });
}

test('the browser quick-start page boots with the README\'s create call', () => {
  const [first] = tsBlocks(README).values();
  assert(first !== undefined, 'the README has no ts block');
  const create = first.split('\n').find((line) => line.includes('OpenGeometry.create('));
  assert(create !== undefined, 'the first ts block of the README has no OpenGeometry.create call');
  const page = readFileSync(QUICK_START_PAGE, 'utf8').split('\n').map((line) => line.trim());
  assert(page.includes(create.trim()), `quick-start.ts does not contain: ${create.trim()}`);
});
