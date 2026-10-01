import { spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const LOG_DIRECTORY = path.join(REPOSITORY_ROOT, '.check');
const WORKER_BUNDLE = path.join(REPOSITORY_ROOT, 'dist/tessellation-worker.js');
const THREE_IMPORT = /(?:\bfrom\s*|\bimport\s*\(?\s*)['"]three(?:\/[^'"]*)?['"]/;
const FLAKY_BROWSER_TEST = 'instance geometry memory returns to baseline after disposal';
const BROWSER_REPEATS = 5;

function crateSteps(name, directory, lintTargets, withTests) {
  const steps = [
    { name: `rust:${name}:fmt`, cwd: directory, commands: [['cargo', 'fmt', '--all', '--', '--check']] },
    {
      name: `rust:${name}:clippy`,
      cwd: directory,
      commands: [[
        'cargo', 'clippy', '--offline', '--target-dir', 'target/clippy', lintTargets, '--', '-D', 'warnings',
      ]],
    },
  ];
  if (!withTests) return steps;
  return [
    ...steps,
    {
      name: `rust:${name}:test`,
      cwd: directory,
      commands: [['cargo', 'test', '--offline', '--all-targets', '--no-fail-fast']],
    },
  ];
}

function npmStep(script) {
  return { name: script, cwd: '.', commands: [['npm', 'run', script]] };
}

function browserStep(threeVersion, portBase) {
  return {
    name: `browser:three-${threeVersion}`,
    cwd: '.',
    threeVersion,
    portBase,
    concurrent: true,
    commands: [['npm', 'run', 'test:browser']],
  };
}

const CHECK_STEPS = [
  ...crateSteps('opengeometry', 'opengeometry', '--all-targets', true),
  ...crateSteps('test-support', 'opengeometry/test-support', '--all-targets', true),
  ...crateSteps('snapshot', 'tools/snapshot', '--all-targets', true),
  ...crateSteps('parity-oracle', 'tools/parity-oracle', '--bins', false),
  { name: 'test:wasm', cwd: 'opengeometry', commands: [['wasm-pack', 'test', '--node']] },
  npmStep('build'),
  { name: 'worker-bundle', check: workerBundleFindings },
  ...['lint:check', 'typecheck', 'check:source', 'check:cycles', 'check:duplicates', 'build-example']
    .map(npmStep),
  ...['test:bindings', 'test:node', 'test:performance'].map(npmStep),
];

const FULL_STEPS = [
  ...CHECK_STEPS,
  browserStep('168', '4175'),
  browserStep('184', '4185'),
];

function workerBundleFindings() {
  if (!existsSync(WORKER_BUNDLE)) return [`missing ${WORKER_BUNDLE}`];
  return readFileSync(WORKER_BUNDLE, 'utf8')
    .split('\n')
    .map((line, index) => ({ line, number: index + 1 }))
    .filter((entry) => THREE_IMPORT.test(entry.line))
    .map((entry) => `dist/tessellation-worker.js:${String(entry.number)} imports three: ${entry.line.trim()}`);
}

function errorText(error) {
  return error instanceof Error ? error.message : String(error);
}

function optionValue(argv, flag) {
  const index = argv.indexOf(flag);
  if (index < 0) return undefined;
  const value = argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${flag} needs a value`);
  return value;
}

function listOption(argv, flag) {
  return optionValue(argv, flag)?.split(',').filter(Boolean);
}

function checkKnown(names, steps) {
  const known = new Set(steps.map((step) => step.name));
  const unknown = names.filter((name) => !known.has(name));
  if (unknown.length === 0) return;
  throw new Error(`unknown step ${unknown.join(', ')}; steps are ${[...known].join(', ')}`);
}

export function selectSteps(argv) {
  const steps = argv.includes('--full') ? FULL_STEPS : CHECK_STEPS;
  const only = listOption(argv, '--only');
  const skip = listOption(argv, '--skip') ?? [];
  const from = optionValue(argv, '--from');
  checkKnown([...(only ?? []), ...skip, ...(from === undefined ? [] : [from])], steps);
  const start = from === undefined ? 0 : steps.findIndex((step) => step.name === from);
  const selected = steps.slice(start)
    .filter((step) => (only === undefined || only.includes(step.name)) && !skip.includes(step.name));
  if (selected.length === 0) throw new Error('nothing selected');
  return selected;
}

function failedTests(lines) {
  const index = lines.findIndex((line) => /^ {2}\d+ failed$/.test(line));
  if (index < 0) return [];
  const count = Number.parseInt(lines[index]?.trim() ?? '', 10);
  return lines.slice(index + 1, index + 1 + count).map((line) => line.replace(/ ─*$/, ''));
}

function browserFlake(log) {
  const lines = log.split('\n');
  const failed = failedTests(lines);
  const onlyFlaky = failed.length === 1 && failed[0]?.endsWith(` › ${FLAKY_BROWSER_TEST}`);
  const outside = lines.some((line) => /^ {2}\d+ interrupted$/.test(line) || line.includes('not a part of any test'));
  return onlyFlaky && !outside ? { verdict: 'KNOWN-FLAKE', detail: FLAKY_BROWSER_TEST } : undefined;
}

export function classifyFlake(stepName, log) {
  if (stepName.startsWith('browser:')) return browserFlake(log);
  return undefined;
}

export function playwrightCounts(log) {
  const count = (label) => Number(new RegExp(`^ {2}(\\d+) ${label}\\b`, 'm').exec(log)?.[1] ?? 0);
  return { passed: count('passed'), failed: count('failed') };
}

function commandEnvironment(step) {
  const env = { ...process.env };
  if (step.threeVersion !== undefined) env.OG_THREE_VERSION = step.threeVersion;
  if (step.portBase !== undefined) env.OG_PW_PORT_BASE = step.portBase;
  return env;
}

function commandStatus(command, step, log) {
  writeSync(log, `$ ${command.join(' ')}\n`);
  return new Promise((resolve) => {
    const child = spawn(command[0], command.slice(1), {
      cwd: path.join(REPOSITORY_ROOT, step.cwd),
      env: commandEnvironment(step),
      stdio: ['ignore', log, log],
    });
    child.on('error', (error) => {
      writeSync(log, `${error.message}\n`);
      resolve(127);
    });
    child.on('close', (code) => resolve(code ?? 127));
  });
}

function checkFindings(check) {
  try {
    return check();
  } catch (error) {
    return [`check threw: ${errorText(error)}`];
  }
}

async function stepStatus(step, log) {
  if (step.check) {
    const findings = checkFindings(step.check);
    writeSync(log, findings.length === 0 ? 'no findings\n' : `${findings.join('\n')}\n`);
    return findings.length === 0 ? 0 : 1;
  }
  for (const command of step.commands) {
    const status = await commandStatus(command, step, log);
    if (status !== 0) return status;
  }
  return 0;
}

async function attempt(step, suffix) {
  const file = path.join(LOG_DIRECTORY, `${step.name}${suffix}.log`);
  const log = openSync(file, 'w');
  const status = await stepStatus(step, log);
  closeSync(log);
  return { status, text: readFileSync(file, 'utf8') };
}

async function browserRepeat(step, status) {
  const repeat = `--repeat-each=${String(BROWSER_REPEATS)}`;
  const commands = [['npm', 'run', 'test:browser', '--', repeat, '--grep', FLAKY_BROWSER_TEST]];
  const rerun = await attempt({ ...step, commands }, '.repeat');
  const counts = playwrightCounts(rerun.text);
  const tally = `${String(counts.passed)} passed, ${String(counts.failed)} failed with ${repeat}`;
  if (counts.passed === 0) return { status, note: `memory test ${tally}` };
  return { status: 0, note: `KNOWN-FLAKE memory test ${tally}` };
}

async function stepOutcome(step) {
  const first = await attempt(step, '');
  if (first.status === 0) return { status: 0, note: '' };
  const flake = classifyFlake(step.name, first.text);
  if (flake === undefined) return { status: first.status, note: '' };
  return browserRepeat(step, first.status);
}

async function runStep(step) {
  const started = performance.now();
  const outcome = await stepOutcome(step);
  const seconds = ((performance.now() - started) / 1000).toFixed(1);
  const verdict = outcome.status === 0 ? 'ok  ' : 'FAIL';
  const note = outcome.note === '' ? '' : ` ${outcome.note}`;
  process.stdout.write(`${verdict} ${step.name.padEnd(28)} exit ${String(outcome.status)} ${seconds}s${note}\n`);
  return { name: step.name, ...outcome };
}

function stepBatches(steps) {
  const batches = [];
  for (const step of steps) {
    const last = batches.at(-1);
    if (step.concurrent && last?.[0]?.concurrent) last.push(step);
    else batches.push([step]);
  }
  return batches;
}

async function runSteps(steps) {
  const results = [];
  for (const batch of stepBatches(steps)) results.push(...await Promise.all(batch.map(runStep)));
  return results;
}

async function main(argv) {
  let steps;
  try {
    steps = selectSteps(argv);
  } catch (error) {
    process.stderr.write(`verify: ${errorText(error)}\n`);
    process.exitCode = 1;
    return;
  }
  mkdirSync(LOG_DIRECTORY, { recursive: true });
  const results = await runSteps(steps);
  const failed = results.filter((result) => result.status !== 0).map((result) => result.name);
  const flakes = results.filter((result) => result.status === 0 && result.note.startsWith('KNOWN-FLAKE'));
  const flakeNote = flakes.length === 0 ? '' : `; known flakes: ${flakes.map((result) => result.name).join(', ')}`;
  process.stdout.write(failed.length === 0
    ? `${String(steps.length)} steps passed${flakeNote}; logs in .check/\n`
    : `failed: ${failed.join(', ')}${flakeNote}; logs in .check/\n`);
  process.exitCode = failed.length === 0 ? 0 : 1;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) await main(process.argv.slice(2));
