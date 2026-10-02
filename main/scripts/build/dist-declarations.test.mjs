import assert from 'node:assert/strict';
import { test } from 'node:test';
import path from 'node:path';
import ts from 'typescript';
import { REPOSITORY_ROOT, relative } from '../lib/paths.mjs';

const ENTRY = path.join(REPOSITORY_ROOT, 'dist', 'index.d.ts');
const OPTIONS = {
  noEmit: true,
  skipLibCheck: false,
  target: ts.ScriptTarget.ES2022,
  lib: ['lib.es2022.d.ts', 'lib.dom.d.ts'],
  types: [],
  module: ts.ModuleKind.Node16,
  moduleResolution: ts.ModuleResolutionKind.Node16,
};

function describe(diagnostic) {
  const file = diagnostic.file === undefined ? '' : `${relative(diagnostic.file.fileName)} `;
  return `${file}TS${String(diagnostic.code)}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')}`;
}

test('the built declarations resolve under node16', () => {
  const program = ts.createProgram([ENTRY], OPTIONS);
  assert.deepEqual(program.getOptionsDiagnostics().map(describe), []);
  const distFiles = program.getSourceFiles().filter((file) => relative(file.fileName).startsWith('dist/'));
  const diagnostics = distFiles
    .flatMap((file) => [...program.getSyntacticDiagnostics(file), ...program.getSemanticDiagnostics(file)]);
  assert.deepEqual(diagnostics.map(describe), []);
  assert(distFiles.length > 1);
});
