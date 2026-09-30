import { defineConfig } from 'vite';

const ROOT = decodeURIComponent(new URL('.', import.meta.url).pathname);

export default defineConfig({
  root: ROOT,
  ...(process.env.OG_THREE_VERSION === '184' ? { resolve: { alias: { three: 'three-184' } } } : {}),
  publicDir: decodeURIComponent(new URL('../../dist', import.meta.url).pathname),
  build: {
    target: 'esnext',
    outDir: '../examples-dist',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: `${ROOT}/index.html`,
        sweep: `${ROOT}/sweep.html`,
        boolean: `${ROOT}/boolean.html`,
        analyticBoolean: `${ROOT}/operations/analytic-boolean-operations.html`,
        opening: `${ROOT}/opening.html`,
        cuboid: `${ROOT}/shapes/cuboid.html`,
        cylinder: `${ROOT}/shapes/cylinder.html`,
        stepExport: `${ROOT}/operations/step-export.html`,
      },
    },
  },
  optimizeDeps: { exclude: ['opengeometry'] },
});
