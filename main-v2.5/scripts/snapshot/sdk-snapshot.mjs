import { OpenGeometry } from '../../dist/index.js';
import { createInlineKernel } from '../lib/kernel.mjs';
import { allGetters, exportStep, geometry } from './sdk-dump.mjs';
import { buildScene, copyAndDispose, editScene, pickWall, stepLog, trialEdit } from './sdk-scene.mjs';
import { outcome, outcomeAsync, prepareDirectory, writer } from './stable-output.mjs';

const EXPORTS = [
  { name: 'level-default', options: (parts) => ({ nodes: [parts.level] }) },
  { name: 'level-metre-y', options: (parts) => ({ nodes: [parts.level], unit: 'metre', upAxis: 'Y' }) },
  {
    name: 'bodies-millimetre-z',
    options: (parts) => ({ nodes: [parts.wall, 'rail-2'], unit: 'millimetre', upAxis: 'Z' }),
  },
  { name: 'wire-rejected', options: (parts) => ({ nodes: [parts.profile] }) },
  { name: 'invalid-unit', options: (parts) => ({ nodes: [parts.level], unit: 'inch' }) },
];

function members(parts, copy) {
  const { level, profile, wall, cutter, path, disc, rail } = parts;
  return [level, profile, wall, cutter, path, disc, rail, copy].filter(Boolean);
}

async function writeExports(out, parts) {
  for (const { name, options } of EXPORTS) {
    const result = await exportStep(options(parts));
    if (result.value) {
      await out.text(`export.${name}.step`, result.value.text);
      await out.json(`export.${name}.report`, result.value.report);
    } else {
      await out.json(`export.${name}.error`, result.error);
    }
  }
}

async function writeBodies(out, bodies) {
  for (const body of bodies) {
    await out.json(`brep.${body.ogId}`, outcome(() => body.getBrep()));
  }
}

async function main() {
  const directory = process.argv[2];
  if (!directory) throw new Error('usage: node scripts/snapshot/sdk-snapshot.mjs <out-dir>');
  await prepareDirectory(directory);
  const out = writer(directory);
  await createInlineKernel();
  const log = stepLog();
  const parts = buildScene(log);
  await out.json('nodes.after-build', allGetters(members(parts)));
  editScene(log, parts);
  trialEdit(log, parts);
  await out.json('nodes.after-edit', allGetters(members(parts)));
  const copy = copyAndDispose(log, parts);
  parts.scene.updateMatrixWorld(true);
  log.run('flush sync', () => OpenGeometry.flush({ geometry: 'sync' }));
  await out.json('geometry', geometry([parts.wall, parts.rail, copy]));
  await out.json('settled', await outcomeAsync(() => OpenGeometry.settled()));
  await out.json('pick', pickWall(parts));
  await writeExports(out, parts);
  await out.json('nodes.final', allGetters(members(parts, copy)));
  await writeBodies(out, [parts.profile, parts.wall, parts.path, parts.disc, parts.rail, copy]);
  await out.json('mark-stats', outcome(() => OpenGeometry.markStats()));
  await out.json('steps', log.entries);
}

await main();
