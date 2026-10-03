import type { TestInfo } from '@playwright/test';
import { spawn } from 'node:child_process';
import { closeSync, openSync } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

const REPOSITORY_ROOT = path.resolve(import.meta.dirname, '../../../..');
const MANIFEST = path.join(REPOSITORY_ROOT, 'opengeometry', 'Cargo.toml');

export type StepFiles = { step: string; report: string; log: string };

export async function writeStep(
  testInfo: TestInfo,
  name: string,
  exported: { text: string; report: unknown },
): Promise<StepFiles> {
  const files = {
    step: testInfo.outputPath(`${name}.step`),
    report: testInfo.outputPath(`${name}.report.json`),
    log: testInfo.outputPath(`${name}.check.log`),
  };
  await writeFile(files.step, exported.text);
  await writeFile(files.report, JSON.stringify(exported.report));
  return files;
}

export function part21Check(args: string[], log: string): Promise<number> {
  const fd = openSync(log, 'w');
  const command = ['run', '--offline', '--manifest-path', MANIFEST, '--example', 'part21_check', '--', ...args];
  const child = spawn('cargo', command, { cwd: REPOSITORY_ROOT, env: process.env, stdio: ['ignore', fd, fd] });
  return new Promise<number>((resolve, reject) => {
    child.on('error', reject);
    child.on('close', (code) => { resolve(code ?? -1); });
  }).finally(() => { closeSync(fd); });
}
