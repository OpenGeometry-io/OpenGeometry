import { spawnSync } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const LOG_DIRECTORY = path.join(REPOSITORY_ROOT, '.check');
const WORKER_BUNDLE = path.join(REPOSITORY_ROOT, 'dist/tessellation-worker.js');
const THREE_IMPORT = /(?:\bfrom\s*|\bimport\s*\(?\s*)['"]three(?:\/[^'"]*)?['"]/;

function crateSteps(name, directory, root, lintTargets, withTests) {
  const steps = [
    { name: `rust:${name}:fmt`, cwd: directory, commands: [['cargo', 'fmt', '--all', '--', '--check']] },
    {
      name: `rust:${name}:clippy`,
      cwd: directory,
      commands: [['touch', root], ['cargo', 'clippy', '--offline', lintTargets, '--', '-D', 'warnings']],
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

function browserStep(threeVersion) {
  return { name: `browser:three-${threeVersion}`, cwd: '.', threeVersion, commands: [['npm', 'run', 'test:browser']] };
}

const CHECK_STEPS = [
  ...crateSteps('opengeometry', 'opengeometry', 'src/lib.rs', '--all-targets', true),
  ...crateSteps('test-support', 'opengeometry/test-support', 'src/lib.rs', '--all-targets', true),
  ...crateSteps('snapshot', 'tools/snapshot', 'src/main.rs', '--all-targets', true),
  ...crateSteps('parity-oracle', 'tools/parity-oracle', 'src/main.rs', '--bins', false),
  { name: 'wasm:test', cwd: 'opengeometry', commands: [['wasm-pack', 'test', '--node']] },
  npmStep('build'),
  { name: 'worker-bundle', check: workerBundleFindings },
  ...['lint:check', 'typecheck', 'check:source', 'check:cycles', 'check:duplicates', 'build-example']
    .map(npmStep),
  ...['test:bindings', 'test:node', 'test:performance'].map(npmStep),
];

const FULL_STEPS = [
  ...CHECK_STEPS,
  browserStep('168'),
  browserStep('184'),
];

function workerBundleFindings() {
  if (!existsSync(WORKER_BUNDLE)) return [`missing ${WORKER_BUNDLE}`];
  return readFileSync(WORKER_BUNDLE, 'utf8')
    .split('\n')
    .map((line, index) => ({ line, number: index + 1 }))
    .filter((entry) => THREE_IMPORT.test(entry.line))
    .map((entry) => `dist/tessellation-worker.js:${String(entry.number)} imports three: ${entry.line.trim()}`);
}

function optionValue(argv, flag) {
  const index = argv.indexOf(flag);
  if (index < 0) return undefined;
  const value = argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${flag} needs a value`);
  return value;
}

function checkKnown(names, steps) {
  const known = new Set(steps.map((step) => step.name));
  const unknown = names.filter((name) => !known.has(name));
  if (unknown.length === 0) return;
  throw new Error(`unknown step ${unknown.join(', ')}; steps are ${[...known].join(', ')}`);
}

function selectedSteps(argv) {
  const steps = argv.includes('--full') ? FULL_STEPS : CHECK_STEPS;
  const only = optionValue(argv, '--only')?.split(',').filter(Boolean);
  const from = optionValue(argv, '--from');
  checkKnown([...(only ?? []), ...(from === undefined ? [] : [from])], steps);
  const start = from === undefined ? 0 : steps.findIndex((step) => step.name === from);
  return steps.slice(start).filter((step) => only === undefined || only.includes(step.name));
}

function commandEnvironment(step) {
  const env = { ...process.env };
  if (step.threeVersion !== undefined) env.OG_THREE_VERSION = step.threeVersion;
  return env;
}

function commandStatus(command, step, log) {
  writeSync(log, `$ ${command.join(' ')}\n`);
  const result = spawnSync(command[0], command.slice(1), {
    cwd: path.join(REPOSITORY_ROOT, step.cwd),
    env: commandEnvironment(step),
    stdio: ['ignore', log, log],
  });
  if (result.error) writeSync(log, `${result.error.message}\n`);
  return result.status ?? 127;
}

function stepStatus(step, log) {
  if (step.check) {
    const findings = step.check();
    writeSync(log, findings.length === 0 ? 'no findings\n' : `${findings.join('\n')}\n`);
    return findings.length === 0 ? 0 : 1;
  }
  for (const command of step.commands) {
    const status = commandStatus(command, step, log);
    if (status !== 0) return status;
  }
  return 0;
}

function runStep(step) {
  const started = performance.now();
  const log = openSync(path.join(LOG_DIRECTORY, `${step.name}.log`), 'w');
  const status = stepStatus(step, log);
  closeSync(log);
  const seconds = ((performance.now() - started) / 1000).toFixed(1);
  const verdict = status === 0 ? 'ok  ' : 'FAIL';
  process.stdout.write(`${verdict} ${step.name.padEnd(28)} exit ${String(status)} ${seconds}s\n`);
  return status;
}

const STEPS = selectedSteps(process.argv.slice(2));
mkdirSync(LOG_DIRECTORY, { recursive: true });
const FAILED = STEPS.filter((step) => runStep(step) !== 0).map((step) => step.name);
process.stdout.write(FAILED.length === 0
  ? `${String(STEPS.length)} steps passed; logs in .check/\n`
  : `failed: ${FAILED.join(', ')}; logs in .check/\n`);
process.exitCode = FAILED.length === 0 ? 0 : 1;
