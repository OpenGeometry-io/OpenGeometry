import { punctuatorEnd, skipLineComment, tokenize } from './tokenize.mjs';

const IDENTIFIER_START = /[A-Za-z_]/;
const IDENTIFIER_PART = /[A-Za-z0-9_]/;
const NUMBER = /^(?:0[xob][0-9a-fA-F_]+|[0-9][0-9_]*(?:\.[0-9][0-9_]*)?(?:[eE][+-]?[0-9_]+)?)(?:[a-z][a-z0-9]*)?/;
const PUNCTUATORS = ['<<=', '>>=', '...', '..=', '::', '->', '=>', '==', '!=', '<=', '>=', '&&', '||', '+=', '-=', '*=',
  '/=', '%=', '^=', '&=', '|=', '<<', '>>', '..'];

function skipBlockComment(source, index) {
  let depth = 0;
  let cursor = index;
  while (cursor < source.length) {
    if (source.startsWith('/*', cursor)) {
      depth += 1;
      cursor += 2;
    } else if (source.startsWith('*/', cursor)) {
      depth -= 1;
      cursor += 2;
      if (depth === 0) return cursor;
    } else cursor += 1;
  }
  return cursor;
}

function quotedEnd(source, index) {
  let cursor = index + 1;
  while (cursor < source.length && source[cursor] !== '"') cursor += source[cursor] === '\\' ? 2 : 1;
  return cursor + 1;
}

function rawStringEnd(source, index) {
  const match = /^[bc]?r(#*)"/.exec(source.slice(index, index + 260));
  if (!match) return -1;
  const closing = `"${match[1]}`;
  const end = source.indexOf(closing, index + match[0].length);
  return end === -1 ? source.length : end + closing.length;
}

function quoteEnd(source, index) {
  if (source[index + 1] === '\\') {
    const end = source.indexOf("'", index + 2 + (source[index + 2] === "'" ? 1 : 0));
    return { end: end + 1, kind: 'literal' };
  }
  const width = source.codePointAt(index + 1) > 0xffff ? 2 : 1;
  if (source[index + 1 + width] === "'") return { end: index + 2 + width, kind: 'literal' };
  let cursor = index + 1;
  while (cursor < source.length && IDENTIFIER_PART.test(source[cursor])) cursor += 1;
  return { end: cursor, kind: 'lifetime' };
}

function literalEnd(source, index) {
  const raw = rawStringEnd(source, index);
  if (raw !== -1) return raw;
  if (/^[bc]"/.test(source.slice(index, index + 2))) return quotedEnd(source, index + 1);
  if (source.startsWith("b'", index)) return quoteEnd(source, index + 1).end;
  return -1;
}

function nextToken(source, index) {
  const char = source[index];
  if (source.startsWith('//', index)) return { skip: skipLineComment(source, index) };
  if (source.startsWith('/*', index)) return { skip: skipBlockComment(source, index) };
  const literal = literalEnd(source, index);
  if (literal !== -1) return { end: literal, kind: 'literal' };
  if (char === '"') return { end: quotedEnd(source, index), kind: 'literal' };
  if (char === "'") return quoteEnd(source, index);
  if (IDENTIFIER_START.test(char)) {
    let cursor = index + (source.startsWith('r#', index) && IDENTIFIER_START.test(source[index + 2] ?? '') ? 2 : 0);
    while (cursor < source.length && IDENTIFIER_PART.test(source[cursor])) cursor += 1;
    return { end: cursor, kind: 'identifier' };
  }
  const number = NUMBER.exec(source.slice(index, index + 64));
  if (number) return { end: index + number[0].length, kind: 'literal' };
  return { end: punctuatorEnd(source, index, PUNCTUATORS), kind: 'punctuator' };
}

export function lexRust(source) {
  return tokenize(source, nextToken);
}
