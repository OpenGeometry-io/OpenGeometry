const RELATIVE_FROM = /(\bfrom\s*)(['"])(\.{1,2}\/[^'"]*)\2/g;
const EXTENSION = /\.[A-Za-z0-9]+$/;

export function withJsSpecifiers(source) {
  return source.replace(RELATIVE_FROM, (clause, from, quote, specifier) => {
    if (EXTENSION.test(specifier.split('/').at(-1) ?? '')) return clause;
    return `${from}${quote}${specifier}.js${quote}`;
  });
}
