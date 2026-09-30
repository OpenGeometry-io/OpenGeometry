import { nodeResolve } from '@rollup/plugin-node-resolve';
import typescript from '@rollup/plugin-typescript';

export default [
  {
    input: 'opengeometry-three/index.ts',
    external: ['three', /^three\//],
    output: { file: 'dist/index.js', format: 'esm', sourcemap: true },
    plugins: [nodeResolve(), typescript({ tsconfig: './tsconfig.json' })],
  },
  {
    input: 'opengeometry-three/src/testing/internals.ts',
    external: ['three', /^three\//],
    output: { file: 'dist/testing.js', format: 'esm', sourcemap: true },
    plugins: [
      nodeResolve(),
      typescript({ tsconfig: './tsconfig.json', declaration: false, declarationDir: undefined }),
    ],
  },
  {
    input: 'opengeometry-three/src/rendering/tessellation-worker.ts',
    output: { file: 'dist/tessellation-worker.js', format: 'esm', sourcemap: true },
    plugins: [nodeResolve(), typescript({ tsconfig: './tsconfig.worker.json' })],
  },
];
