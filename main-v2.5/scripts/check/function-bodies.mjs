const CONTROL_KEYWORDS = new Set(['if', 'for', 'while', 'switch', 'catch', 'with', 'return', 'typeof', 'await',
  'yield']);
const ANNOTATION_STOPS = new Set(['=', 'class', 'extends', 'implements', 'interface', 'return', 'const', 'let', 'var',
  'new', 'import', 'export']);

export function matchingClose(tokens, open) {
  const pairs = { '{': '}', '(': ')', '[': ']' };
  const opener = tokens[open].text;
  const closer = pairs[opener];
  let depth = 0;
  for (let index = open; index < tokens.length; index += 1) {
    if (tokens[index].text === opener) depth += 1;
    else if (tokens[index].text === closer) {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return tokens.length - 1;
}

export function matchingOpen(tokens, close) {
  const pairs = { '}': '{', ')': '(', ']': '[' };
  const closer = tokens[close].text;
  const opener = pairs[closer];
  let depth = 0;
  for (let index = close; index >= 0; index -= 1) {
    if (tokens[index].text === closer) depth += 1;
    else if (tokens[index].text === opener) {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return 0;
}

export function topLevelDelimiter(tokens, start) {
  let depth = 0;
  for (let index = start; index < tokens.length; index += 1) {
    const text = tokens[index].text;
    if (text === '(' || text === '[') depth += 1;
    else if (text === ')' || text === ']') depth -= 1;
    else if (depth === 0 && (text === ';' || text === '{')) return index;
  }
  return -1;
}

function rustBodyOpen(tokens, fnIndex) {
  const delimiter = topLevelDelimiter(tokens, fnIndex + 1);
  return delimiter !== -1 && tokens[delimiter].text === '{' ? delimiter : -1;
}

export function rustFunctionOpens(tokens) {
  const opens = [];
  tokens.forEach((token, index) => {
    if (token.text !== 'fn') return;
    if (tokens[index + 1]?.kind !== 'identifier') return;
    const open = rustBodyOpen(tokens, index);
    if (open !== -1) opens.push(open);
  });
  return opens;
}

function annotatedParen(tokens, open) {
  if (tokens[open - 1]?.text === ')') return open - 1;
  let depth = 0;
  for (let index = open - 1; index > 0 && index > open - 200; index -= 1) {
    const text = tokens[index].text;
    if ([')', ']', '}', '>'].includes(text)) depth += 1;
    else if (['(', '[', '{', '<'].includes(text)) depth -= 1;
    if (depth < 0 || (depth === 0 && (text === ';' || ANNOTATION_STOPS.has(text)))) return -1;
    if (depth === 0 && text === ':' && tokens[index - 1].text === ')') return index - 1;
  }
  return -1;
}

function isTypeScriptFunctionOpen(tokens, open) {
  if (tokens[open - 1]?.text === '=>') return true;
  const close = annotatedParen(tokens, open);
  if (close === -1) return false;
  const before = tokens[matchingOpen(tokens, close) - 1];
  if (!before) return false;
  if (before.text === 'function' || before.text === '>' || before.text === '?') return true;
  return before.kind === 'identifier' && !CONTROL_KEYWORDS.has(before.text);
}

export function typeScriptFunctionOpens(tokens) {
  const opens = [];
  tokens.forEach((token, index) => {
    if (token.text === '{' && isTypeScriptFunctionOpen(tokens, index)) opens.push(index);
  });
  return opens;
}
