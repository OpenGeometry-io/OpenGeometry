import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const packageRoot = resolve(__dirname, "..");

const sourceWasm = resolve(packageRoot, "../opengeometry/pkg/opengeometry_bg.wasm");
const sourceWasmTypes = resolve(packageRoot, "../opengeometry/pkg/opengeometry_bg.wasm.d.ts");
const targetDir = resolve(packageRoot, "examples-dist/assets/wasm");

mkdirSync(targetDir, { recursive: true });

if (!existsSync(sourceWasm)) {
  throw new Error(`WASM source not found at ${sourceWasm}`);
}

copyFileSync(sourceWasm, resolve(targetDir, "opengeometry_bg.wasm"));

if (existsSync(sourceWasmTypes)) {
  copyFileSync(sourceWasmTypes, resolve(targetDir, "opengeometry_bg.wasm.d.ts"));
}

console.log(`Copied wasm assets to ${targetDir}`);
