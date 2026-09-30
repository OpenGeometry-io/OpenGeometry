import { readFileSync } from 'node:fs';
import path from 'node:path';
import { ESLint } from 'eslint';
import { RESTRICTED_SYNTAX, lintConfig } from '../../eslint.config.js';
import { REPOSITORY_ROOT, relative } from '../lib/paths.mjs';
import EXCEPTIONS from './lint-exceptions.json' with { type: 'json' };

const RETURN_TYPE_RULES = new Set([
  '@typescript-eslint/explicit-function-return-type',
  '@typescript-eslint/explicit-module-boundary-types',
]);
const SYNTAX_GROUP_BY_MESSAGE = new Map(Object.entries(RESTRICTED_SYNTAX)
  .flatMap(([group, options]) => options.map((option) => [option.message, group])));

async function unsuppressedMessages() {
  const eslint = new ESLint({ cwd: REPOSITORY_ROOT, overrideConfigFile: true, overrideConfig: lintConfig([], []) });
  const results = await eslint.lintFiles(['.']);
  return results.flatMap((result) => result.messages
    .map((message) => ({ ...message, file: relative(result.filePath) })));
}

function suppressionKey(message) {
  const group = message.ruleId === 'no-restricted-syntax' ? SYNTAX_GROUP_BY_MESSAGE.get(message.message) : undefined;
  return group ? `syntax:${group}` : message.ruleId;
}

function staleEntryParts(entry, index, found) {
  const keys = [...(entry.rules ?? []), ...(entry.syntax ?? []).map((group) => `syntax:${group}`)];
  const label = `lint exception ${String(index + 1)} (${entry.owner})`;
  const staleFiles = entry.files
    .filter((file) => !keys.some((key) => found.has(`${file} ${key}`)))
    .map((file) => `${label}: ${file} no longer needs ${keys.join(', ')}`);
  const staleKeys = keys
    .filter((key) => !entry.files.some((file) => found.has(`${file} ${key}`)))
    .map((key) => `${label}: ${key} no longer suppresses anything in its files`);
  return [...staleFiles, ...staleKeys];
}

function offsetOf(text, line, column) {
  const before = text.split('\n').slice(0, line - 1).join('\n');
  return (line > 1 ? before.length + 1 : 0) + column - 1;
}

function reportedText(message) {
  const text = readFileSync(path.join(REPOSITORY_ROOT, message.file), 'utf8');
  const end = offsetOf(text, message.endLine ?? message.line, message.endColumn ?? message.column);
  return text.slice(offsetOf(text, message.line, message.column), end);
}

function staleReturnTypeNames(messages) {
  const heads = messages
    .filter((message) => RETURN_TYPE_RULES.has(message.ruleId ?? '') && message.messageId === 'missingReturnType')
    .map(reportedText);
  return EXCEPTIONS.returnTypeNames.names
    .filter((name) => !heads.some((head) => new RegExp(`\\b${name}\\b`).test(head)))
    .map((name) => `returnTypeNames: ${name} no longer needs an exemption`);
}

const MESSAGES = await unsuppressedMessages();
const FOUND = new Set(MESSAGES.map((message) => `${message.file} ${suppressionKey(message)}`));
const STALE = [
  ...EXCEPTIONS.exceptions.flatMap((entry, index) => staleEntryParts(entry, index, FOUND)),
  ...staleReturnTypeNames(MESSAGES),
];
for (const stale of STALE) process.stdout.write(`${stale}\n`);
const CHECKED = `${String(EXCEPTIONS.exceptions.length)} lint exceptions and `
  + `${String(EXCEPTIONS.returnTypeNames.names.length)} return-type names checked`;
process.stdout.write(`${CHECKED}, ${String(STALE.length)} stale\n`);
process.exitCode = STALE.length > 0 ? 1 : 0;
