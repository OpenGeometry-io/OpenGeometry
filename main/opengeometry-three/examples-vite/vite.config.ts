import { defineConfig } from 'vite';

const ROOT = decodeURIComponent(new URL('.', import.meta.url).pathname);

export default defineConfig({
  root: ROOT,
  cacheDir: `../../node_modules/.vite/examples-three-${process.env.OG_THREE_VERSION ?? '168'}`,
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
        roundedProfile: `${ROOT}/shapes/rounded-profile.html`,
        arcPathSweep: `${ROOT}/operations/sweep-arc-path.html`,
        curvedMeetsStraight: `${ROOT}/operations/curved-meets-straight.html`,
        stepExport: `${ROOT}/operations/step-export.html`,
        slotPrism: `${ROOT}/shapes/slot-prism.html`,
        roundedBox: `${ROOT}/shapes/rounded-box.html`,
        pipeSegment: `${ROOT}/shapes/pipe-segment.html`,
        polysolid: `${ROOT}/shapes/polysolid.html`,
        wireKinds: `${ROOT}/shapes/wire-kinds.html`,
      },
    },
  },
  optimizeDeps: { exclude: ['opengeometry'] },
});
