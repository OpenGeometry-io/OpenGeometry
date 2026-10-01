import { nodeResolve } from '@rollup/plugin-node-resolve';
import typescript from '@rollup/plugin-typescript';

export default [
  {
    input: { index: 'opengeometry-three/index.ts', testing: 'opengeometry-three/src/testing/internals.ts' },
    external: ['three', /^three\//],
    output: { dir: 'dist', format: 'esm', sourcemap: true, entryFileNames: '[name].js' },
    plugins: [nodeResolve(), typescript({ tsconfig: './tsconfig.json' })],
  },
  {
    input: 'opengeometry-three/src/rendering/tessellation-worker.ts',
    output: { file: 'dist/tessellation-worker.js', format: 'esm', sourcemap: true },
    plugins: [nodeResolve(), typescript({ tsconfig: './tsconfig.worker.json' })],
  },
];
