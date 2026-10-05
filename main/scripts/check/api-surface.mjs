import path from 'node:path';
import ts from 'typescript';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const ENTRY = path.join(REPOSITORY_ROOT, 'dist', 'index.d.ts');
const DIST_ROOT = `${path.join(REPOSITORY_ROOT, 'dist')}${path.sep}`;
const PRINTER = ts.createPrinter({ removeComments: true });

const PROGRAM = ts.createProgram([ENTRY], {
  noEmit: true,
  skipLibCheck: true,
  target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
});
const CHECKER = PROGRAM.getTypeChecker();

function normalise(text) {
  return text.replace(/\s+/g, ' ').replace(/ ;/g, ';').trim();
}

function print(node) {
  return normalise(PRINTER.printNode(ts.EmitHint.Unspecified, node, node.getSourceFile()));
}

function resolved(symbol) {
  return symbol.flags & ts.SymbolFlags.Alias ? CHECKER.getAliasedSymbol(symbol) : symbol;
}

function isPrivate(member) {
  const modifiers = ts.canHaveModifiers(member) ? ts.getModifiers(member) ?? [] : [];
  const privateModifier = modifiers.some((modifier) => modifier.kind === ts.SyntaxKind.PrivateKeyword);
  return privateModifier || (member.name !== undefined && ts.isPrivateIdentifier(member.name));
}

function header(node, keyword) {
  const typeParameters = node.typeParameters ? `<${node.typeParameters.map(print).join(', ')}>` : '';
  const heritage = (node.heritageClauses ?? []).map(print).join(' ');
  return normalise(`${keyword} ${node.name?.text ?? 'default'}${typeParameters} ${heritage}`);
}

function describeMembers(node, keyword) {
  const members = node.members.filter((member) => !isPrivate(member)).map(print).sort();
  return [header(node, keyword), ...members.map((member) => `  ${member}`)];
}

function withoutExport(text) {
  return text.replace(/^(export\s+)?(default\s+)?(declare\s+)?/, '');
}

function describeDeclaration(node) {
  if (ts.isClassDeclaration(node)) return describeMembers(node, 'class');
  if (ts.isInterfaceDeclaration(node)) return describeMembers(node, 'interface');
  if (ts.isVariableDeclaration(node)) {
    const list = node.parent;
    const keyword = list.flags & ts.NodeFlags.Const ? 'const' : list.flags & ts.NodeFlags.Let ? 'let' : 'var';
    return [`${keyword} ${print(node)}`];
  }
  return [withoutExport(print(node))];
}

function localDeclarations(symbol) {
  return (symbol.declarations ?? [])
    .filter((declaration) => declaration.getSourceFile().fileName.startsWith(DIST_ROOT));
}

function referencedSymbols(node, found) {
  const visit = (child) => {
    let name;
    if (ts.isTypeReferenceNode(child)) name = child.typeName;
    else if (ts.isExpressionWithTypeArguments(child)) name = child.expression;
    else if (ts.isTypeQueryNode(child)) name = child.exprName;
    if (name && ts.isIdentifier(name)) {
      const symbol = CHECKER.getSymbolAtLocation(name);
      if (symbol) found.add(resolved(symbol));
    }
    ts.forEachChild(child, visit);
  };
  visit(node);
}

function describeSymbol(label, symbol, pending) {
  const declarations = localDeclarations(symbol);
  const blocks = declarations.map((declaration) => {
    referencedSymbols(declaration, pending);
    return describeDeclaration(declaration).map((line) => `  ${line}`).join('\n');
  });
  return `${label}\n${blocks.sort().join('\n')}`;
}

function surface() {
  const entryFile = PROGRAM.getSourceFile(ENTRY);
  if (!entryFile) throw new Error(`${ENTRY} is missing; build the SDK first`);
  const moduleSymbol = CHECKER.getSymbolAtLocation(entryFile);
  if (!moduleSymbol) throw new Error(`${ENTRY} is not a module`);
  const exported = CHECKER.getExportsOfModule(moduleSymbol);
  const emitted = new Set(exported.map(resolved));
  const pending = new Set();
  const sections = exported
    .map((symbol) => describeSymbol(`export ${symbol.getName()}`, resolved(symbol), pending));
  const referenced = [];
  for (const symbol of pending) {
    if (emitted.has(symbol) || localDeclarations(symbol).length === 0) continue;
    emitted.add(symbol);
    referenced.push(describeSymbol(`referenced ${symbol.getName()}`, symbol, pending));
  }
  return [...sections.sort(), ...referenced.sort()].join('\n');
}

process.stdout.write(`${surface()}\n`);
