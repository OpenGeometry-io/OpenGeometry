import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { REPOSITORY_ROOT, listFiles, relative } from '../lib/paths.mjs';
import { lexRust } from './lexers/rust-lexer.mjs';
import { quotedEnd, skipBlockComment } from './lexers/tokenize.mjs';
import CONFIG from './source-rules.json' with { type: 'json' };

const KEBAB_CASE = new RegExp(CONFIG.kebabCase);
const KIND_BY_EXTENSION = new Map(Object.entries(CONFIG.fileTypes)
  .flatMap(([kind, extensions]) => extensions.map((extension) => [extension, kind])));
const COMMENT_START = /\/\/|\/\*/g;
const TOML_STRING_OR_COMMENT = /"""[\s\S]*?"""|'''[\s\S]*?'''|"(?:\\.|[^"\\\n])*"|'[^'\n]*'|#/g;
const LIST_COMMENT = /^\s*(?:#|\/\/)/;
const FORBIDDEN_MENTION = new RegExp(CONFIG.forbiddenMentions.join('|'), 'gi');
const COMMENT_SKIPPED_DIRECTORIES = [...CONFIG.skipDirectories, ...CONFIG.vendoredDirectories];

function kindOf(file) {
  return KIND_BY_EXTENSION.get(path.extname(file));
}

function lineAt(text, index) {
  return text.slice(0, index).split('\n').length;
}

function syntaxComments(source) {
  const lines = new Set();
  const visit = (node) => {
    if (ts.isJSDoc(node)) return;
    const children = node.getChildren(source);
    if (children.length === 0) {
      const trivia = source.text.slice(node.pos, node.getStart(source));
      for (const match of trivia.matchAll(COMMENT_START)) lines.add(lineAt(source.text, node.pos + match.index));
    }
    for (const child of children) visit(child);
  };
  visit(source);
  return [...lines];
}

function scriptComments(text, file) {
  return syntaxComments(ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true));
}

function jsonComments(text, file) {
  return syntaxComments(ts.parseJsonText(file, text));
}

function styleComments(text) {
  const lines = [];
  let index = 0;
  while (index < text.length) {
    if (text[index] === '"' || text[index] === "'") index = quotedEnd(text, index);
    else if (text.startsWith('/*', index)) {
      lines.push(lineAt(text, index));
      index = skipBlockComment(text, index);
    } else index += 1;
  }
  return lines;
}

function tagEnd(text, index) {
  let cursor = index + 1;
  while (cursor < text.length && text[cursor] !== '>') {
    cursor = text[cursor] === '"' || text[cursor] === "'" ? quotedEnd(text, cursor) : cursor + 1;
  }
  return cursor + 1;
}

function embeddedComments(text, index, element) {
  const open = tagEnd(text, index);
  const close = text.toLowerCase().indexOf(`</${element}`, open);
  const end = close === -1 ? text.length : close;
  const body = text.slice(open, end);
  const inner = element === 'script' ? scriptComments(body, 'inline.js') : styleComments(body);
  const offset = lineAt(text, open) - 1;
  return { lines: inner.map((line) => line + offset), end };
}

function markupComments(text) {
  const lines = [];
  let index = 0;
  while (index < text.length) {
    const element = /^<(script|style)\b/i.exec(text.slice(index, index + 8))?.[1]?.toLowerCase();
    if (text.startsWith('<!--', index)) {
      lines.push(lineAt(text, index));
      const end = text.indexOf('-->', index + 4);
      index = end === -1 ? text.length : end + 3;
    } else if (element) {
      const embedded = embeddedComments(text, index, element);
      lines.push(...embedded.lines);
      index = embedded.end;
    } else index = text[index] === '<' ? tagEnd(text, index) : index + 1;
  }
  return lines;
}

function opensComment(text, index) {
  return text.startsWith('//', index) || text.startsWith('/*', index);
}

function closesGap(text, from, index) {
  const gap = text.slice(from, index);
  return !opensComment(text, index) && (/^\s*$/.test(gap) || lexRust(`${gap} x`).length === 1);
}

function tokenStart(text, from, token) {
  let index = text.indexOf(token, from);
  while (!closesGap(text, from, index)) index = text.indexOf(token, index + 1);
  return index;
}

function gapComments(text, from, to) {
  return [...text.slice(from, to).matchAll(COMMENT_START)].map((match) => lineAt(text, from + match.index));
}

function rustComments(text) {
  const lines = new Set();
  let from = 0;
  for (const token of lexRust(text)) {
    const start = tokenStart(text, from, token.text);
    for (const line of gapComments(text, from, start)) lines.add(line);
    from = start + token.text.length;
  }
  for (const line of gapComments(text, from, text.length)) lines.add(line);
  return [...lines];
}

function tomlComments(text) {
  return [...text.matchAll(TOML_STRING_OR_COMMENT)]
    .filter((match) => match[0] === '#')
    .map((match) => lineAt(text, match.index));
}

function listComments(text) {
  return text.split('\n').flatMap((line, index) => (LIST_COMMENT.test(line) ? [index + 1] : []));
}

const COMMENT_FINDERS = {
  script: scriptComments, json: jsonComments, style: styleComments, markup: markupComments,
  rust: rustComments, toml: tomlComments, list: listComments,
};

function commentViolations(file, kind = kindOf(file)) {
  const text = readFileSync(file, 'utf8');
  const finder = COMMENT_FINDERS[kind];
  return finder(text, file).map((line) => `${relative(file)}:${line} has a comment`);
}

function lengthViolations(file) {
  const text = readFileSync(file, 'utf8');
  const lines = text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
  return lines > CONFIG.maxFileLines ? [`${relative(file)} has ${lines} lines, above ${CONFIG.maxFileLines}`] : [];
}

function mentionViolations(file) {
  const text = readFileSync(file, 'utf8');
  const lines = new Set([...text.matchAll(FORBIDDEN_MENTION)].map((match) => lineAt(text, match.index)));
  return [...lines].map((line) => `${relative(file)}:${line} has a forbidden mention`);
}

function namingViolations(file) {
  const segments = relative(file).split('/');
  const badDirectory = segments.slice(0, -1).find((segment) => !KEBAB_CASE.test(segment));
  if (badDirectory) return [`${relative(file)} has a directory name that is not kebab-case: ${badDirectory}`];
  const name = path.basename(file);
  return name.split('.').every((part) => KEBAB_CASE.test(part)) ? [] : [`${relative(file)} is not kebab-case`];
}

function textFiles() {
  return readdirSync(REPOSITORY_ROOT, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile())
    .map((entry) => path.join(entry.parentPath, entry.name))
    .filter((file) => !relative(file).split('/').some((segment) => CONFIG.skipDirectories.includes(segment)))
    .filter((file) => !readFileSync(file).includes(0));
}

const FILES = listFiles(CONFIG.roots, [...KIND_BY_EXTENSION.keys()], CONFIG.skipDirectories).sort();
const RUST_FILES = listFiles(CONFIG.rustRoots, ['.rs'], COMMENT_SKIPPED_DIRECTORIES).sort();
const TOML_FILES = listFiles(CONFIG.roots, ['.toml'], COMMENT_SKIPPED_DIRECTORIES).sort();
const LIST_FILES = listFiles(CONFIG.exceptionListRoots, ['.txt'], CONFIG.skipDirectories).sort();
const TEXT_FILES = textFiles().sort();
const VIOLATIONS = [
  ...FILES.flatMap((file) => [...namingViolations(file), ...commentViolations(file)]),
  ...RUST_FILES.flatMap((file) => [...commentViolations(file, 'rust'), ...lengthViolations(file)]),
  ...TOML_FILES.flatMap((file) => commentViolations(file, 'toml')),
  ...LIST_FILES.flatMap((file) => commentViolations(file, 'list')),
  ...TEXT_FILES.flatMap(mentionViolations),
];
const CHECKED = new Set([...FILES, ...RUST_FILES, ...TOML_FILES, ...LIST_FILES, ...TEXT_FILES]);
for (const violation of VIOLATIONS) process.stdout.write(`${violation}\n`);
process.stdout.write(`${CHECKED.size} files checked, ${VIOLATIONS.length} source rule violations\n`);
process.exitCode = VIOLATIONS.length > 0 ? 1 : 0;
