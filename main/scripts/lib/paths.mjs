import { readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export function relative(file) {
  return path.relative(REPOSITORY_ROOT, file).split(path.sep).join('/');
}

export function listFiles(roots, extensions, skippedDirectories) {
  return roots
    .flatMap((root) => readdirSync(path.join(REPOSITORY_ROOT, root.path), {
      withFileTypes: true, recursive: root.recursive,
    }))
    .filter((entry) => entry.isFile() && extensions.includes(path.extname(entry.name)))
    .map((entry) => path.join(entry.parentPath, entry.name))
    .filter((file) => !relative(file).split('/').some((segment) => skippedDirectories.includes(segment)));
}
