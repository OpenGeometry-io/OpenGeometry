import { rmSync } from 'node:fs';
import path from 'node:path';
import { REPOSITORY_ROOT } from '../lib/paths.mjs';

rmSync(path.join(REPOSITORY_ROOT, 'dist'), { recursive: true, force: true });
