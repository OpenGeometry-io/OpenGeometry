import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { REPOSITORY_ROOT, listFiles, relative } from '../lib/paths.mjs';
import { matchingClose, rustFunctionOpens, typeScriptFunctionOpens } from './function-bodies.mjs';
import { lexRust } from './lexers/rust-lexer.mjs';
import { lexTypeScript } from './lexers/typescript-lexer.mjs';

const ALLOWED_FILE = path.join(REPOSITORY_ROOT, 'scripts', 'check', 'duplicates-allowed.json');
const MINIMUM_LINES = 8;
const RUST_ROOTS = ['opengeometry/src', 'opengeometry/examples', 'opengeometry/tests', 'opengeometry/test-support/src',
  'opengeometry/test-support/tests', 'tools/snapshot/src', 'tools/parity-oracle/src']
  .map((directory) => ({ path: directory, recursive: true }));
const SCRIPT_ROOTS = [
  { path: '.', recursive: false },
  ...['scripts', 'opengeometry-three/src', 'opengeometry-three/tests', 'opengeometry-three/examples-vite']
    .map((directory) => ({ path: directory, recursive: true })),
];
const SCRIPT_EXTENSIONS = ['.ts', '.mjs', '.js'];
const RUST_WORDS = {
  literals: new Set(['true', 'false']),
  keywords: new Set(['as', 'async', 'await', 'break', 'const', 'continue', 'crate', 'dyn', 'else', 'enum', 'extern',
    'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'pub', 'ref', 'return', 'self',
    'Self', 'static', 'struct', 'super', 'trait', 'type', 'union', 'unsafe', 'use', 'where', 'while']),
};
const SCRIPT_WORDS = {
  literals: new Set(['true', 'false', 'null', 'undefined']),
  keywords: new Set(['abstract', 'any', 'as', 'async', 'await', 'bigint', 'boolean', 'break', 'case', 'catch', 'class',
    'const', 'continue', 'debugger', 'declare', 'default', 'delete', 'do', 'else', 'enum', 'export', 'extends',
    'finally', 'for', 'from', 'function', 'get', 'if', 'implements', 'import', 'in', 'infer', 'instanceof', 'interface',
    'is', 'keyof', 'let', 'never', 'new', 'number', 'object', 'of', 'override', 'private', 'protected', 'public',
    'readonly', 'return', 'satisfies', 'set', 'static', 'string', 'super', 'switch', 'symbol', 'this', 'throw', 'try',
    'type', 'typeof', 'unknown', 'var', 'void', 'while', 'yield']),
};

function files(roots, extensions) {
  return listFiles(roots, extensions, []).filter((file) => !file.endsWith('.d.ts')).sort();
}

function normalisedBody(tokens, words) {
  const names = new Map();
  return tokens.map((token) => {
    if (token.kind === 'literal' || (token.kind === 'identifier' && words.literals.has(token.text))) return '<literal>';
    if (token.kind !== 'identifier' || words.keywords.has(token.text)) return token.text;
    if (!names.has(token.text)) names.set(token.text, `<name-${names.size}>`);
    return names.get(token.text);
  }).join(' ');
}

function bodies(file, tokens, opens, words) {
  return opens.flatMap((open) => {
    const close = matchingClose(tokens, open);
    if (tokens[close].line - tokens[open].line + 1 < MINIMUM_LINES) return [];
    const normalised = normalisedBody(tokens.slice(open, close + 1), words);
    const sha256 = createHash('sha256').update(normalised).digest('hex');
    return [{ sha256, location: `${relative(file)}:${tokens[open].line}` }];
  });
}

function rustBodies() {
  return files(RUST_ROOTS, ['.rs']).flatMap((file) => {
    const tokens = lexRust(readFileSync(file, 'utf8'));
    return bodies(file, tokens, rustFunctionOpens(tokens), RUST_WORDS);
  });
}

function scriptBodies() {
  return files(SCRIPT_ROOTS, SCRIPT_EXTENSIONS).flatMap((file) => {
    const tokens = lexTypeScript(readFileSync(file, 'utf8'));
    return bodies(file, tokens, typeScriptFunctionOpens(tokens), SCRIPT_WORDS);
  });
}

function duplicateGroups() {
  const byHash = new Map();
  for (const body of [...rustBodies(), ...scriptBodies()]) {
    byHash.set(body.sha256, [...(byHash.get(body.sha256) ?? []), body.location]);
  }
  return [...byHash.entries()]
    .filter(([, members]) => members.length > 1)
    .map(([sha256, members]) => ({ sha256, count: members.length, members }))
    .sort((a, b) => a.members[0].localeCompare(b.members[0]) || a.sha256.localeCompare(b.sha256));
}

function allowedCounts() {
  const allowed = JSON.parse(readFileSync(ALLOWED_FILE, 'utf8'));
  return new Map(allowed.groups.map((group) => [group.sha256, group.count]));
}

function report(groups, allowed) {
  let unexpected = 0;
  for (const group of groups) {
    const permitted = (allowed.get(group.sha256) ?? 0) >= group.count;
    if (!permitted) unexpected += 1;
    process.stdout.write(`${permitted ? 'allowed' : 'NEW'} ${group.sha256} x${group.count}\n`);
    for (const member of group.members) process.stdout.write(`  ${member}\n`);
  }
  process.stdout.write(`${groups.length} duplicate groups, ${unexpected} not in duplicates-allowed.json\n`);
  return unexpected;
}

process.exitCode = report(duplicateGroups(), allowedCounts()) > 0 ? 1 : 0;
