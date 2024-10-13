import { nodeResolve } from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import babel from '@rollup/plugin-babel';
import typescript from '@rollup/plugin-typescript';

export default {
  input: 'main/opengeometry-three/index.ts',
  output: {
    file: 'dist/index.js',
    format: 'esm',
    name: 'opengeometry',
    sourcemap: true,
  },
  plugins: [
    nodeResolve(),
    commonjs(),
    typescript({
      tsconfig: './tsconfig.json', // Use your project's tsconfig
      declaration: true,
      declarationDir: 'dist/', // Outputs declarations in a specific folder
    }),
    babel({
      babelHelpers: 'bundled',
      exclude: 'node_modules/**',
    }),
  ],
};
