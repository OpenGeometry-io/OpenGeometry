import { readdirSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const examplesRoot = resolve(__dirname, "examples-vite");

function collectHtmlFiles(dir, files = []) {
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    const stats = statSync(fullPath);
    if (stats.isDirectory()) {
      collectHtmlFiles(fullPath, files);
      continue;
    }

    if (extname(fullPath) === ".html") {
      files.push(fullPath);
    }
  }

  return files;
}

const htmlInputs = collectHtmlFiles(examplesRoot);
const input = Object.fromEntries(
  htmlInputs.map((filePath) => {
    const id = relative(examplesRoot, filePath)
      .replace(/\\/g, "/")
      .replace(/\.html$/, "");
    return [id, filePath];
  })
);

export default defineConfig({
  root: examplesRoot,
  base: "./",
  build: {
    outDir: resolve(__dirname, "examples-dist"),
    emptyOutDir: true,
    rollupOptions: { input },
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
