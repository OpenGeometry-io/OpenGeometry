import { spawn } from 'node:child_process';
import path from 'node:path';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

const DUMP_VARIABLE = 'OG_GOLDEN_DUMP';
const COMPARING_TEST = 'every_golden_scene_matches_the_recorded_golden_for_this_target';
const CARGO_ARGUMENTS = [
  'test', '--locked', '--offline', '--manifest-path', 'opengeometry/Cargo.toml', '--test', 'golden',
  '--', COMPARING_TEST, '--exact', '--nocapture',
];

function main() {
  const directory = process.argv[2];
  if (!directory) throw new Error('usage: npm run snapshot:kernel -- <out-dir>');
  const child = spawn('cargo', CARGO_ARGUMENTS, {
    cwd: REPOSITORY_ROOT,
    env: { ...process.env, [DUMP_VARIABLE]: path.resolve(directory) },
    stdio: ['ignore', 1, 2],
  });
  child.on('error', (error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 127;
  });
  child.on('close', (code) => {
    process.exitCode = code ?? 127;
  });
}

main();
