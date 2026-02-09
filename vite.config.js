import { defineConfig } from 'vite';

export default defineConfig({
  root: 'test',
  server: {
    port: 7070,
    fs: {
      // Allow serving files from one level up to the project root
      allow: ['..']
    }
  },
  resolve: {
    alias: {
      three: '../node_modules/three/build/three.module.js'
    }
  }
});