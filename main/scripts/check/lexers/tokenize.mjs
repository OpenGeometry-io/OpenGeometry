export function skipLineComment(source, index) {
  const end = source.indexOf('\n', index);
  return end === -1 ? source.length : end;
}

export function skipBlockComment(source, index) {
  const end = source.indexOf('*/', index + 2);
  return end === -1 ? source.length : end + 2;
}

export function quotedEnd(source, index) {
  const quote = source[index];
  let cursor = index + 1;
  while (cursor < source.length && source[cursor] !== quote) cursor += source[cursor] === '\\' ? 2 : 1;
  return cursor + 1;
}

export function punctuatorEnd(source, index, punctuators) {
  const match = punctuators.find((candidate) => source.startsWith(candidate, index));
  return index + (match ? match.length : 1);
}

export function tokenize(source, nextToken) {
  const tokens = [];
  let index = 0;
  let line = 1;
  while (index < source.length) {
    if (/\s/.test(source[index])) {
      if (source[index] === '\n') line += 1;
      index += 1;
      continue;
    }
    const token = nextToken(source, index, tokens[tokens.length - 1]);
    const end = token.skip ?? token.end;
    if (token.kind) tokens.push({ text: source.slice(index, end), kind: token.kind, line });
    for (let cursor = index; cursor < end; cursor += 1) if (source[cursor] === '\n') line += 1;
    index = end;
  }
  return tokens;
}
