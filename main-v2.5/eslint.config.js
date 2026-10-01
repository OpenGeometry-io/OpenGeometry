import js from '@eslint/js';
import rawPlugin from '@typescript-eslint/eslint-plugin/use-at-your-own-risk/raw-plugin';
import EXCEPTIONS from './scripts/check/lint-exceptions.json' with { type: 'json' };

const SDK = 'opengeometry-three/src';
const SDK_FILES = [`${SDK}/**/*.ts`, 'opengeometry-three/index.ts'];
const CONSUMER_FILES = ['examples-vite', 'tests'].map((home) => `opengeometry-three/${home}/**/*.{ts,js,mjs}`);
const CONSUMER_MESSAGE = 'Examples and tests use the built SDK, never src/.';
const SDK_SOURCE_IMPORTS = ['(^|/)src/', '(^|/)(\\.\\.|opengeometry-three)(/|/index(\\.[jt]s)?)?$'];
const JS_FILES = ['**/*.js', '**/*.mjs'];
const WORKER_ENTRY = `${SDK}/rendering/tessellation-worker.ts`;

const LAYERS = [
  { name: 'dto', files: [`${SDK}/dto/**/*.ts`], imports: ['(^|/)dto/'] },
  { name: 'errors', files: [`${SDK}/errors.ts`], imports: ['(^|/)errors(\\.js)?$'] },
  {
    name: 'constants',
    files: [`${SDK}/constants/**/*.ts`, `${SDK}/limits.ts`],
    imports: ['(^|/)constants/', '(^|/)limits(\\.js)?$'],
  },
  {
    name: 'runtime-state',
    files: [`${SDK}/runtime/runtime-state.ts`, `${SDK}/runtime/event-bus.ts`],
    imports: ['(^|/)runtime-state(\\.js)?$', '(^|/)event-bus(\\.js)?$'],
  },
  { name: 'kernel', files: [`${SDK}/kernel/**/*.ts`], imports: ['(^|/)kernel/'] },
  { name: 'world-graph', files: [`${SDK}/world-graph/**/*.ts`], imports: ['(^|/)world-graph/'] },
  { name: 'rendering', files: [`${SDK}/rendering/**/*.ts`], imports: ['(^|/)rendering/'] },
  { name: 'marks', files: [`${SDK}/marks/**/*.ts`], imports: ['(^|/)marks/'] },
  { name: 'bodies', files: [`${SDK}/bodies/**/*.ts`], imports: ['(^|/)bodies/'] },
  {
    name: 'picking-export',
    files: [`${SDK}/picking/**/*.ts`, `${SDK}/export/**/*.ts`],
    imports: ['(^|/)picking/', '(^|/)export/'],
  },
  {
    name: 'runtime',
    files: [`${SDK}/runtime/create-runtime.ts`, `${SDK}/runtime/reset.ts`],
    imports: ['(^|/)create-runtime(\\.js)?$', '(^|/)reset(\\.js)?$'],
  },
  { name: 'facade', files: [`${SDK}/opengeometry.ts`], imports: ['^(\\.{1,2}/)+(src/)?opengeometry(\\.js)?$'] },
  {
    name: 'entries',
    files: ['opengeometry-three/index.ts', `${SDK}/testing/**/*.ts`],
    imports: ['^(\\.\\./)*\\.\\.(/index(\\.js)?)?$', '(^|/)testing(/|$)'],
  },
];
const THREE_AND_DOM_FREE = new Set(['dto', 'errors', 'constants', 'runtime-state', 'kernel', 'world-graph']);
const THREE_IMPORTS = [{ regex: '^three(/|$)', message: 'This layer imports no three.' }];
const GLUE_IMPORT = { regex: '(^|/)opengeometry/pkg/', message: 'Only kernel/** imports the WASM glue.' };
const WORKER_IMPORTS = [
  ...THREE_IMPORTS,
  { regex: '(^|/)(bodies|runtime)/', message: 'The worker imports no three, bodies or runtime.' },
];
const WORKER_BARRED_LAYERS = new Set(['bodies', 'runtime']);
const DOM_MESSAGE = 'This layer uses no DOM.';
const DOM_NAMES = [
  'window', 'self', 'document', 'navigator', 'location', 'history', 'screen', 'frames', 'parent', 'top', 'opener',
  'localStorage', 'sessionStorage', 'indexedDB', 'caches', 'customElements', 'devicePixelRatio', 'innerWidth',
  'innerHeight', 'addEventListener', 'removeEventListener', 'dispatchEvent', 'postMessage', 'onmessage',
  'requestAnimationFrame', 'cancelAnimationFrame', 'requestIdleCallback', 'cancelIdleCallback', 'getComputedStyle',
  'matchMedia', 'alert', 'confirm', 'prompt', 'createImageBitmap', 'importScripts',
  'Window', 'Document', 'Navigator', 'Node', 'Element', 'HTMLElement', 'HTMLCanvasElement', 'HTMLImageElement',
  'SVGElement', 'Event', 'CustomEvent', 'EventTarget', 'MessageEvent', 'ErrorEvent', 'MessageChannel', 'MessagePort',
  'BroadcastChannel', 'Worker', 'SharedWorker', 'Image', 'ImageData', 'ImageBitmap', 'OffscreenCanvas', 'Path2D',
  'CanvasRenderingContext2D', 'OffscreenCanvasRenderingContext2D', 'WebGLRenderingContext', 'WebGL2RenderingContext',
  'ResizeObserver', 'MutationObserver', 'IntersectionObserver', 'DOMParser', 'XMLHttpRequest', 'FileReader',
];
const DOM_TYPES = DOM_NAMES.filter((name) => /^[A-Z]/.test(name));
const DOM_GLOBALS = DOM_NAMES.map((name) => ({ name, message: DOM_MESSAGE }));
const DOM_PROPERTIES = DOM_NAMES.map((property) => ({ object: 'globalThis', property, message: DOM_MESSAGE }));

const MODULE_STATE_MESSAGE = 'No module-level mutable state outside runtime/runtime-state.ts and the worker entry.';
const MODULE_DECLARATORS = ['Program', 'Program > ExportNamedDeclaration']
  .map((parent) => `${parent} > VariableDeclaration > VariableDeclarator`);
const STATIC_FIELD = ':matches(PropertyDefinition, AccessorProperty)[static=true]';
const MUTABLE_VALUE = ':matches(NewExpression, ObjectExpression, ArrayExpression, CallExpression)';
const TYPE_WRAPPER = ':matches(TSAsExpression, TSTypeAssertion, TSSatisfiesExpression, TSNonNullExpression)'
  + ":not([typeAnnotation.typeName.name='const'])";

function mutableInitializers(holder, field) {
  return [
    `${holder} > ${MUTABLE_VALUE}.${field}`,
    `${holder} > ${TYPE_WRAPPER}.${field} > ${MUTABLE_VALUE}.expression`,
    `${holder} > ${TYPE_WRAPPER}.${field} > ${TYPE_WRAPPER}.expression > ${MUTABLE_VALUE}.expression`,
  ];
}

export const RESTRICTED_SYNTAX = {
  'json-parse': [{
    selector: "MemberExpression:matches([object.name='JSON'], [object.property.name='JSON'])"
      + ":matches([property.name='parse'], [property.value='parse'])",
    message: 'JSON.parse only in world-graph/codec.ts and the worker message code.',
  }],
  'og-error-throw': [
    { selector: "ThrowStatement > NewExpression:not([callee.name='OGError'])", message: 'Every throw is an OGError.' },
    { selector: 'ThrowStatement > :not(NewExpression, Identifier)', message: 'Every throw is an OGError.' },
  ],
  'module-state': [
    "Program > VariableDeclaration[kind!='const']",
    "Program > ExportNamedDeclaration > VariableDeclaration[kind!='const']",
    `${STATIC_FIELD}:not([readonly=true])`,
    ...MODULE_DECLARATORS.flatMap((declarator) => mutableInitializers(declarator, 'init')),
    ...mutableInitializers(STATIC_FIELD, 'value'),
  ].map((selector) => ({ selector, message: MODULE_STATE_MESSAGE })),
  'dom-types': [{
    selector: `TSTypeReference > Identifier.typeName[name=/^(HTML|SVG)|^(${DOM_TYPES.join('|')})$/]`,
    message: DOM_MESSAGE,
  }],
};
const SYNTAX_HOMES = [
  { file: `${SDK}/world-graph/codec.ts`, allows: ['json-parse'] },
  { file: `${SDK}/kernel/kernel-errors.ts`, allows: ['json-parse'] },
  { file: `${SDK}/runtime/runtime-state.ts`, allows: ['module-state'] },
  { file: WORKER_ENTRY, allows: ['json-parse', 'module-state'] },
];

function layerOf(file) {
  return LAYERS.find((layer) => layer.files.some((pattern) => pattern === file
    || (pattern.endsWith('/**/*.ts') && file.startsWith(pattern.slice(0, -'**/*.ts'.length)))));
}

function syntaxOptions(layer, allows) {
  const groups = ['og-error-throw', 'json-parse', 'module-state'];
  if (THREE_AND_DOM_FREE.has(layer.name)) groups.push('dom-types');
  return ['error', ...groups.filter((group) => !allows.has(group)).flatMap((group) => RESTRICTED_SYNTAX[group])];
}

function syntaxAllowances(exceptions) {
  const allowances = new Map();
  const allow = (file, names) => {
    allowances.set(file, new Set([...(allowances.get(file) ?? []), ...names]));
  };
  for (const home of SYNTAX_HOMES) allow(home.file, home.allows);
  for (const entry of exceptions) {
    for (const file of entry.files) if (entry.syntax) allow(file, entry.syntax);
  }
  return allowances;
}

function syntaxConfigs(exceptions) {
  const layerWide = LAYERS.map((layer) => ({
    files: layer.files,
    rules: { 'no-restricted-syntax': syntaxOptions(layer, new Set()) },
  }));
  const allowed = [...syntaxAllowances(exceptions)].map(([file, allows]) => ({
    files: [file],
    rules: { 'no-restricted-syntax': syntaxOptions(layerOf(file), allows) },
  }));
  return [...layerWide, ...allowed];
}

function importOptions(layer, index, extra, barred) {
  const upper = LAYERS.slice(index + 1)
    .filter((above) => !barred.has(above.name))
    .flatMap((above) => above.imports.map((regex) => ({
      regex,
      allowTypeImports: true,
      message: `Layer order (D21): ${layer.name} may not value-import ${above.name}.`,
    })));
  const patterns = [...upper, ...extra];
  if (layer.name !== 'kernel') patterns.push(GLUE_IMPORT);
  if (THREE_AND_DOM_FREE.has(layer.name)) patterns.push(...THREE_IMPORTS);
  return ['error', { patterns }];
}

function importConfigs() {
  return LAYERS.flatMap((layer, index) => [
    {
      files: layer.files,
      ignores: layer.name === 'rendering' ? [WORKER_ENTRY] : [],
      rules: { '@typescript-eslint/no-restricted-imports': importOptions(layer, index, [], new Set()) },
    },
    ...(layer.name === 'rendering'
      ? [{
        files: [WORKER_ENTRY],
        rules: {
          '@typescript-eslint/no-restricted-imports': importOptions(layer, index, WORKER_IMPORTS, WORKER_BARRED_LAYERS),
        },
      }]
      : []),
    ...(THREE_AND_DOM_FREE.has(layer.name)
      ? [{
        files: layer.files,
        rules: {
          'no-restricted-globals': ['error', ...DOM_GLOBALS],
          'no-restricted-properties': ['error', ...DOM_PROPERTIES],
        },
      }]
      : []),
  ]);
}

function specifierSelectors(regex) {
  const pattern = `/${regex.replaceAll('/', '\\/')}/i`;
  return [
    `ImportExpression > Literal.source[value=${pattern}]`,
    `ImportExpression > TemplateLiteral.source[expressions.length=0] > TemplateElement[value.cooked=${pattern}]`,
    `TSImportType > TSLiteralType.argument > Literal.literal[value=${pattern}]`,
  ];
}

function consumerConfig() {
  return {
    files: CONSUMER_FILES,
    rules: {
      '@typescript-eslint/no-restricted-imports': ['error', {
        patterns: SDK_SOURCE_IMPORTS.map((regex) => ({ regex, message: CONSUMER_MESSAGE })),
      }],
      'no-restricted-syntax': [
        'error',
        ...SDK_SOURCE_IMPORTS.flatMap(specifierSelectors).map((selector) => ({ selector, message: CONSUMER_MESSAGE })),
      ],
    },
  };
}

function exceptionConfigs(exceptions) {
  return exceptions
    .filter((entry) => entry.rules)
    .map((entry) => ({ files: entry.files, rules: Object.fromEntries(entry.rules.map((rule) => [rule, 'off'])) }));
}

const NAMING = [
  'error',
  { selector: 'default', format: ['camelCase'] },
  { selector: 'typeLike', format: ['PascalCase'] },
  { selector: 'enumMember', format: ['PascalCase'] },
  { selector: 'variable', modifiers: ['const', 'global'], format: ['UPPER_CASE'] },
  { selector: 'import', modifiers: ['namespace'], format: ['UPPER_CASE'] },
  { selector: 'import', modifiers: ['default'], format: ['camelCase', 'PascalCase'] },
  { selector: 'parameter', format: ['camelCase'], leadingUnderscore: 'allow' },
  {
    selector: ['objectLiteralProperty', 'typeProperty'],
    format: ['camelCase'],
    leadingUnderscore: 'allowDouble',
  },
  { selector: 'objectLiteralProperty', filter: { regex: '^module_or_path$', match: true }, format: null },
  {
    selector: ['objectLiteralProperty', 'typeProperty', 'objectLiteralMethod'],
    modifiers: ['requiresQuotes'],
    format: null,
  },
];
const KERNEL_KEY_FILES = [`${SDK}/dto/brep.ts`, `${SDK}/dto/boolean-report.ts`];
const KERNEL_KEY_NAMING = [
  ...NAMING,
  { selector: 'typeProperty', filter: { regex: '_', match: true }, format: ['snake_case'] },
];

function sharedRules(returnTypeNames) {
  return {
    'no-undef': 'off',
    'max-len': ['error', { code: 120 }],
    'max-lines': ['error', { max: 400 }],
    'max-lines-per-function': ['error', { max: 60 }],
    'max-classes-per-file': ['error', 1],
    '@typescript-eslint/naming-convention': NAMING,
    '@typescript-eslint/no-explicit-any': 'error',
    '@typescript-eslint/no-unsafe-type-assertion': 'error',
    '@typescript-eslint/no-non-null-assertion': 'error',
    '@typescript-eslint/no-floating-promises': 'error',
    '@typescript-eslint/switch-exhaustiveness-check': 'error',
    '@typescript-eslint/only-throw-error': 'error',
    '@typescript-eslint/consistent-type-imports': 'error',
    '@typescript-eslint/explicit-module-boundary-types': ['error', { allowedNames: returnTypeNames }],
    '@typescript-eslint/explicit-function-return-type': [
      'error',
      { allowExpressions: true, allowedNames: returnTypeNames },
    ],
    '@typescript-eslint/consistent-type-definitions': ['error', 'type'],
    '@typescript-eslint/no-extraneous-class': ['error', { allowStaticOnly: true }],
  };
}

export function lintConfig(exceptions, returnTypeNames) {
  return [
    {
      ignores: [
        'dist/**', 'opengeometry-three/examples-dist/**', 'opengeometry/**', 'tools/**', 'test-results/**',
        'playwright-report/**', '**/*.d.ts',
      ],
    },
    js.configs.recommended,
    ...rawPlugin.flatConfigs['flat/strict-type-checked'],
    ...rawPlugin.flatConfigs['flat/stylistic-type-checked'],
    {
      languageOptions: {
        parserOptions: {
          projectService: { allowDefaultProject: ['playwright.config.ts'], defaultProject: 'tsconfig.tests.json' },
          tsconfigRootDir: import.meta.dirname,
        },
      },
      rules: sharedRules(returnTypeNames),
    },
    { files: JS_FILES, ...rawPlugin.flatConfigs['flat/disable-type-checked'] },
    {
      files: JS_FILES,
      rules: {
        '@typescript-eslint/explicit-module-boundary-types': 'off',
        '@typescript-eslint/explicit-function-return-type': 'off',
        '@typescript-eslint/naming-convention': NAMING,
      },
    },
    { files: SDK_FILES, rules: { 'max-lines': ['error', { max: 300 }] } },
    { files: KERNEL_KEY_FILES, rules: { '@typescript-eslint/naming-convention': KERNEL_KEY_NAMING } },
    ...importConfigs(),
    ...syntaxConfigs(exceptions),
    consumerConfig(),
    ...exceptionConfigs(exceptions),
  ];
}

export default lintConfig(EXCEPTIONS.exceptions, EXCEPTIONS.returnTypeNames.names);
