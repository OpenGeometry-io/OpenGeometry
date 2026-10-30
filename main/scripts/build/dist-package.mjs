const GITHUB = 'https://github.com/OpenGeometry-io/OpenGeometry';
const RAW = 'https://raw.githubusercontent.com/OpenGeometry-io/OpenGeometry/main';
const ABSOLUTE = /^(?:[a-z][a-z0-9+.-]*:|#|\/\/)/i;
const FENCE = /^ {0,3}(`{3,}|~{3,})[^\n]*\n[\s\S]*?^ {0,3}\1[ \t]*$/gm;
const INLINE_CODE = /(`+)(?!`)[\s\S]*?(?<!`)\1(?!`)/g;
const HELD = /\uE000(\d+)\uE000/g;
const LINK = /(!?)\[((?:[^[\]]|!\[[^[\]]*\]\([^()\s]*\))*)\]\(([^()\s]+)((?:\s+"[^"]*")?)\)/g;
const REFERENCE = /^( {0,3}\[(?!\^)[^\]]+\]:[ \t]*)(\S+)/gm;
const ATTRIBUTE = /(?<=\s)(href|src)="([^"]*)"/g;

export function distManifest(manifest) {
  return {
    name: manifest.name,
    version: manifest.version,
    description: manifest.description,
    keywords: manifest.keywords,
    homepage: manifest.homepage,
    repository: manifest.repository,
    bugs: manifest.bugs,
    license: manifest.license,
    type: 'module',
    main: './index.js',
    types: './index.d.ts',
    exports: {
      '.': { types: './index.d.ts', import: './index.js' },
      './tessellation-worker.js': './tessellation-worker.js',
      './opengeometry_bg.wasm': './opengeometry_bg.wasm',
      './package.json': './package.json',
    },
    peerDependencies: manifest.peerDependencies,
  };
}

export function packageReadme(text) {
  const held = [];
  const hold = (code) => `\uE000${String(held.push(code) - 1)}\uE000`;
  const prose = text.replace(FENCE, hold).replace(INLINE_CODE, hold);
  const rewritten = markdownLinks(prose)
    .replace(REFERENCE, (_whole, label, target) => `${label}${absolute(target, false)}`)
    .replace(ATTRIBUTE, (_whole, name, target) => `${name}="${absolute(target, name === 'src')}"`);
  return restore(rewritten, held).replaceAll('utm_source=github', 'utm_source=npm');
}

function restore(text, held) {
  const restored = text.replace(HELD, (_whole, index) => held[Number(index)] ?? '');
  return restored === text ? restored : restore(restored, held);
}

function markdownLinks(text) {
  return text.replace(LINK, (_whole, bang, label, target, title) => {
    const image = bang === '!';
    return `${bang}[${image ? label : markdownLinks(label)}](${absolute(target, image)}${title})`;
  });
}

function absolute(target, image) {
  if (ABSOLUTE.test(target)) return target;
  if (target.startsWith('../')) throw new Error(`${target} points outside the repository`);
  const relative = target.replace(/^\.?\//, '');
  if (image) return `${RAW}/${relative}`;
  const pathPart = relative.split(/[?#]/)[0] ?? '';
  return `${GITHUB}/${pathPart.endsWith('/') ? 'tree' : 'blob'}/main/${relative}`;
}
