import { defineConfig } from 'vite';

const ROOT = decodeURIComponent(new URL('./pages', import.meta.url).pathname);

export default defineConfig({
  root: ROOT,
  ...(process.env.OG_THREE_VERSION === '184' ? { resolve: { alias: { three: 'three-184' } } } : {}),
  publicDir: decodeURIComponent(new URL('../../../dist', import.meta.url).pathname),
  optimizeDeps: { exclude: ['opengeometry'] },
});
