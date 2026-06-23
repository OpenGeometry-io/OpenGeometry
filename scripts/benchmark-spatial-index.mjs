/* eslint-env node */
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const pkgDir = path.join(rootDir, "main", "opengeometry", "pkg");
const openGeometryJsPath = path.join(pkgDir, "opengeometry.js");
const openGeometryWasmPath = path.join(pkgDir, "opengeometry_bg.wasm");

const readOption = (name, fallback) => {
  const prefix = `--${name}=`;
  const value = process.argv.find((arg) => arg.startsWith(prefix));
  return value ? value.slice(prefix.length) : fallback;
};

const readIntegerOption = (name, fallback) => {
  const value = Number(readOption(name, fallback));
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`Invalid --${name}: expected a positive integer.`);
  }
  return value;
};

const itemCount = readIntegerOption("items", "50000");
const queryCount = readIntegerOption("queries", "100");
const repeatCount = readIntegerOption("repeats", "3");

function createPrng(seed) {
  let state = seed >>> 0;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function createSceneItems(count) {
  const random = createPrng(0x0aabb777 ^ count);
  const ids = new Uint32Array(count);
  const bounds = new Float64Array(count * 6);
  const items = new Array(count);

  for (let index = 0; index < count; index += 1) {
    const x = (random() - 0.5) * 1000;
    const y = (random() - 0.5) * 140;
    const z = (random() - 0.5) * 1000;
    const width = 0.5 + random() * 8;
    const height = 0.5 + random() * 6;
    const depth = 0.5 + random() * 8;
    const minX = x - width * 0.5;
    const minY = y - height * 0.5;
    const minZ = z - depth * 0.5;
    const maxX = x + width * 0.5;
    const maxY = y + height * 0.5;
    const maxZ = z + depth * 0.5;
    const offset = index * 6;

    ids[index] = index + 1;
    bounds[offset] = minX;
    bounds[offset + 1] = minY;
    bounds[offset + 2] = minZ;
    bounds[offset + 3] = maxX;
    bounds[offset + 4] = maxY;
    bounds[offset + 5] = maxZ;
    items[index] = { id: index + 1, minX, minY, minZ, maxX, maxY, maxZ };
  }

  return { bounds, ids, items };
}

function createQueries(count) {
  const random = createPrng(0x5eed1234 ^ count);
  const queries = new Array(count);

  for (let index = 0; index < count; index += 1) {
    const x = (random() - 0.5) * 1000;
    const y = (random() - 0.5) * 140;
    const z = (random() - 0.5) * 1000;
    const radius = 20 + random() * 120;
    queries[index] = {
      minX: x - radius,
      minY: y - radius * 0.5,
      minZ: z - radius,
      maxX: x + radius,
      maxY: y + radius * 0.5,
      maxZ: z + radius,
    };
  }

  return queries;
}

function intersects(item, query) {
  return item.minX <= query.maxX
    && item.maxX >= query.minX
    && item.minY <= query.maxY
    && item.maxY >= query.minY
    && item.minZ <= query.maxZ
    && item.maxZ >= query.minZ;
}

function linearQuery(items, query) {
  const hits = [];
  for (const item of items) {
    if (intersects(item, query)) {
      hits.push(item.id);
    }
  }
  return hits;
}

function percentile(values, p) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * p) - 1))] ?? 0;
}

function measure(fn) {
  const samples = [];
  let value;
  for (let index = 0; index < repeatCount; index += 1) {
    const startedAt = performance.now();
    value = fn();
    samples.push(performance.now() - startedAt);
  }
  return {
    value,
    minMs: Math.min(...samples),
    p50Ms: percentile(samples, 0.5),
    p95Ms: percentile(samples, 0.95),
  };
}

function sameIds(left, right) {
  if (left.length !== right.length) return false;
  const sortedLeft = [...left].sort((a, b) => a - b);
  const sortedRight = [...right].sort((a, b) => a - b);
  return sortedLeft.every((value, index) => value === sortedRight[index]);
}

if (!existsSync(openGeometryJsPath) || !existsSync(openGeometryWasmPath)) {
  throw new Error("OpenGeometry WASM package is missing. Run `npm run build-core` first.");
}

const openGeometry = await import(pathToFileURL(openGeometryJsPath).href);
await openGeometry.default({ module_or_path: readFileSync(openGeometryWasmPath) });

const { OGSpatialIndex } = openGeometry;
if (typeof OGSpatialIndex?.fromFlatArrays !== "function") {
  throw new Error("OGSpatialIndex.fromFlatArrays is not available in the WASM package.");
}

const { bounds, ids, items } = createSceneItems(itemCount);
const queries = createQueries(queryCount);

const build = measure(() => OGSpatialIndex.fromFlatArrays(ids, bounds));
const index = build.value;

const linearSamples = [];
const wasmSamples = [];
let totalHits = 0;

for (const query of queries) {
  const linear = measure(() => linearQuery(items, query));
  const wasm = measure(() => index.queryAabbIds(
    query.minX,
    query.minY,
    query.minZ,
    query.maxX,
    query.maxY,
    query.maxZ,
  ));

  if (!sameIds(linear.value, wasm.value)) {
    throw new Error(
      `Query mismatch: linear=${linear.value.length}, wasm=${wasm.value.length}.`
    );
  }

  totalHits += wasm.value.length;
  linearSamples.push(linear.p50Ms);
  wasmSamples.push(wasm.p50Ms);
}

const linearP50Ms = percentile(linearSamples, 0.5);
const wasmP50Ms = percentile(wasmSamples, 0.5);
const nodeCount = index.nodeCount();

index.free?.();

console.log(JSON.stringify({
  benchmark: "spatial-index",
  itemCount,
  queryCount,
  repeatCount,
  primitiveCount: ids.length,
  nodeCount,
  totalHits,
  buildP50Ms: build.p50Ms,
  linearQueryP50Ms: linearP50Ms,
  wasmQueryP50Ms: wasmP50Ms,
  querySpeedup: linearP50Ms > 0 ? linearP50Ms / Math.max(wasmP50Ms, 0.0001) : 0,
}, null, 2));
