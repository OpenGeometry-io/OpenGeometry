import { readdirSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const examplesRoot = resolve(__dirname, "examples-vite");

function collectFiles(dir, extension, files = []) {
  for (const entry of readdirSync(dir)) {
    if (entry.startsWith(".")) {
      continue;
    }

    const fullPath = join(dir, entry);
    const stats = statSync(fullPath);
    if (stats.isDirectory()) {
      collectFiles(fullPath, extension, files);
      continue;
    }

    if (extname(fullPath) === extension) {
      files.push(fullPath);
    }
  }

  return files;
}

function toPosixPath(path) {
  return path.replace(/\\/g, "/");
}

function createManualChunk(id) {
  const normalized = toPosixPath(id);

  if (normalized.includes("/node_modules/three/examples/jsm/libs/stats.module")) {
    return "vendor-stats";
  }

  if (normalized.includes("/node_modules/three/examples/jsm/")) {
    return "vendor-three-extras";
  }

  if (normalized.includes("/node_modules/three/")) {
    return "vendor-three";
  }

  if (
    normalized.includes("/main/opengeometry-three/index.ts")
    || normalized.includes("/main/opengeometry-three/src/")
    || normalized.includes("/main/opengeometry/pkg/")
  ) {
    return "vendor-opengeometry";
  }

  return undefined;
}

function collectHtmlInputs() {
  const htmlFiles = collectFiles(examplesRoot, ".html");
  const inputs = {};

  for (const htmlPath of htmlFiles) {
    const key = toPosixPath(relative(examplesRoot, htmlPath)).replace(/\.html$/, "");
    inputs[key] = htmlPath;
  }

  return inputs;
}

const input = collectHtmlInputs();

export default defineConfig({
  root: examplesRoot,
  base: "./",
  build: {
    outDir: resolve(__dirname, "examples-dist"),
    emptyOutDir: true,
    rollupOptions: {
      input,
      output: {
        manualChunks: createManualChunk,
      },
    },
  },
  resolve: {
    alias: {
      "@og-three": resolve(__dirname, "index.ts"),
    },
  },
  server: {
    fs: {
      allow: [resolve(__dirname, ".."), __dirname],
    },
  },
});
