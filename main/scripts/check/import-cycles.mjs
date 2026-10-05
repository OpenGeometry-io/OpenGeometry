import path from 'node:path';
import ts from 'typescript';
import { REPOSITORY_ROOT, relative } from '../lib/paths.mjs';

const CONFIGS = [
  'tsconfig.json', 'tsconfig.worker.json', 'tsconfig.examples.json', 'tsconfig.tests.json', 'tsconfig.scripts.json',
];
const FIRST_PARTY = ['opengeometry-three', 'scripts'].map((root) => `${path.join(REPOSITORY_ROOT, root)}${path.sep}`);

function isFirstParty(file) {
  return FIRST_PARTY.some((root) => path.resolve(file).startsWith(root)) && !file.endsWith('.d.ts');
}

function parsedConfig(name) {
  const file = path.join(REPOSITORY_ROOT, name);
  const read = ts.readConfigFile(file, ts.sys.readFile);
  if (read.error) throw new Error(ts.flattenDiagnosticMessageText(read.error.messageText, '\n'));
  return ts.parseJsonConfigFileContent(read.config, ts.sys, path.dirname(file));
}

function isValueImport(statement) {
  if (ts.isImportDeclaration(statement)) return !statement.importClause?.isTypeOnly;
  if (ts.isExportDeclaration(statement)) return Boolean(statement.moduleSpecifier) && !statement.isTypeOnly;
  return false;
}

function valueTargets(source, options) {
  return source.statements
    .filter(isValueImport)
    .map((statement) => statement.moduleSpecifier)
    .filter((specifier) => specifier && ts.isStringLiteral(specifier))
    .map((specifier) => ts.resolveModuleName(specifier.text, source.fileName, options, ts.sys).resolvedModule)
    .filter((resolved) => resolved && isFirstParty(resolved.resolvedFileName))
    .map((resolved) => path.resolve(resolved.resolvedFileName));
}

function importGraph() {
  const graph = new Map();
  for (const name of CONFIGS) {
    const config = parsedConfig(name);
    const program = ts.createProgram(config.fileNames, config.options);
    for (const source of program.getSourceFiles()) {
      if (!isFirstParty(source.fileName)) continue;
      const file = path.resolve(source.fileName);
      graph.set(file, new Set([...(graph.get(file) ?? []), ...valueTargets(source, config.options)]));
    }
  }
  return graph;
}

function stronglyConnected(graph) {
  const index = new Map();
  const low = new Map();
  const stack = [];
  const onStack = new Set();
  const components = [];
  const connect = (node) => {
    index.set(node, index.size);
    low.set(node, index.get(node));
    stack.push(node);
    onStack.add(node);
    for (const next of graph.get(node) ?? []) {
      if (!index.has(next)) {
        connect(next);
        low.set(node, Math.min(low.get(node), low.get(next)));
      } else if (onStack.has(next)) low.set(node, Math.min(low.get(node), index.get(next)));
    }
    if (low.get(node) !== index.get(node)) return;
    const component = [];
    let member;
    do {
      member = stack.pop();
      onStack.delete(member);
      component.push(member);
    } while (member !== node);
    components.push(component);
  };
  for (const node of [...graph.keys()].sort()) if (!index.has(node)) connect(node);
  return components;
}

function cycles(graph) {
  return stronglyConnected(graph)
    .filter((component) => component.length > 1 || graph.get(component[0])?.has(component[0]))
    .map((component) => component.map(relative).sort());
}

const GRAPH = importGraph();
const CYCLES = cycles(GRAPH);
for (const cycle of CYCLES) process.stdout.write(`value-import cycle: ${cycle.join(' -> ')}\n`);
const EDGES = [...GRAPH.values()].reduce((sum, targets) => sum + targets.size, 0);
process.stdout.write(`${GRAPH.size} files, ${EDGES} value imports, ${CYCLES.length} value-import cycles\n`);
process.exitCode = CYCLES.length > 0 ? 1 : 0;
