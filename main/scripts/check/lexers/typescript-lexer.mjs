import { punctuatorEnd, quotedEnd, skipBlockComment, skipLineComment, tokenize } from './tokenize.mjs';

const IDENTIFIER_START = /[A-Za-z_$#]/;
const IDENTIFIER_PART = /[A-Za-z0-9_$]/;
const NUMBER = /^(?:0[xob][0-9a-fA-F_]+n?|(?:[0-9][0-9_]*)?\.?[0-9][0-9_]*(?:[eE][+-]?[0-9_]+)?n?)/;
const PUNCTUATORS = ['>>>=', '...', '===', '!==', '**=', '<<=', '>>=', '>>>', '&&=', '||=', '??=', '=>', '==', '!=',
  '<=', '>=', '&&', '||', '??', '?.', '**', '++', '--', '+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '<<', '>>'];
const REGEX_PREFIX = new Set(['(', ',', '=', ':', '[', '!', '&', '|', '?', '{', '}', ';', '+', '-', '*', '%', '<', '>',
  '~', '^', '=>', '==', '===', '!=', '!==', '&&', '||', '??', 'return', 'typeof', 'case', 'in', 'of', 'new', 'void',
  'throw', 'delete', 'else', 'do', 'yield', 'await']);

function codeEnd(source, index) {
  let depth = 1;
  let cursor = index;
  while (cursor < source.length && depth > 0) {
    const char = source[cursor];
    if (char === "'" || char === '"') cursor = quotedEnd(source, cursor);
    else if (char === '`') cursor = templateEnd(source, cursor);
    else if (source.startsWith('//', cursor)) cursor = skipLineComment(source, cursor);
    else if (source.startsWith('/*', cursor)) cursor = skipBlockComment(source, cursor);
    else {
      if (char === '{') depth += 1;
      if (char === '}') depth -= 1;
      cursor += 1;
    }
  }
  return cursor;
}

function templateEnd(source, index) {
  let cursor = index + 1;
  while (cursor < source.length && source[cursor] !== '`') {
    if (source[cursor] === '\\') cursor += 2;
    else if (source.startsWith('${', cursor)) cursor = codeEnd(source, cursor + 2);
    else cursor += 1;
  }
  return cursor + 1;
}

function regexEnd(source, index) {
  let cursor = index + 1;
  let inClass = false;
  while (cursor < source.length && source[cursor] !== '\n') {
    const char = source[cursor];
    if (char === '\\') cursor += 1;
    else if (char === '[') inClass = true;
    else if (char === ']') inClass = false;
    else if (char === '/' && !inClass) break;
    cursor += 1;
  }
  cursor += 1;
  while (cursor < source.length && /[a-z]/.test(source[cursor])) cursor += 1;
  return cursor;
}

function nextToken(source, index, previous) {
  const char = source[index];
  if (source.startsWith('//', index)) return { skip: skipLineComment(source, index) };
  if (source.startsWith('/*', index)) return { skip: skipBlockComment(source, index) };
  if (char === "'" || char === '"') return { end: quotedEnd(source, index), kind: 'literal' };
  if (char === '`') return { end: templateEnd(source, index), kind: 'literal' };
  if (char === '/' && (previous === undefined || REGEX_PREFIX.has(previous.text))) {
    return { end: regexEnd(source, index), kind: 'literal' };
  }
  if (IDENTIFIER_START.test(char)) {
    let cursor = index + 1;
    while (cursor < source.length && IDENTIFIER_PART.test(source[cursor])) cursor += 1;
    return { end: cursor, kind: 'identifier' };
  }
  const number = /[0-9.]/.test(char) ? NUMBER.exec(source.slice(index, index + 64)) : null;
  if (number && number[0] !== '.') return { end: index + number[0].length, kind: 'literal' };
  return { end: punctuatorEnd(source, index, PUNCTUATORS), kind: 'punctuator' };
}

export function lexTypeScript(source) {
  return tokenize(source, nextToken);
}
