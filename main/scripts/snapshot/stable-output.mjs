import { createHash } from 'node:crypto';
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

function exactNumber(value) {
  if (Number.isInteger(value) && !Object.is(value, -0)) return value;
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, value);
  return `f64:${view.getBigUint64(0).toString(16).padStart(16, '0')}`;
}

export function canonical(value) {
  if (typeof value === 'number') return exactNumber(value);
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}

export function stableJson(value) {
  return `${JSON.stringify(canonical(value), null, 1)}\n`;
}

export function digest(view) {
  const bytes = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
  return createHash('sha256').update(bytes).digest('hex');
}

export function outcome(fn) {
  try {
    return { value: fn() };
  } catch (error) {
    return { error: describeError(error) };
  }
}

export async function outcomeAsync(fn) {
  try {
    return { value: await fn() };
  } catch (error) {
    return { error: describeError(error) };
  }
}

export function describeError(error) {
  if (error && typeof error.toJSON === 'function') return error.toJSON();
  return { name: error?.name ?? 'Error', message: String(error?.message ?? error) };
}

export async function prepareDirectory(directory) {
  await mkdir(directory, { recursive: true });
  if ((await readdir(directory)).length > 0) {
    throw new Error(`output directory ${directory} must be empty`);
  }
}

export function writer(directory) {
  return {
    json: (name, value) => writeFile(path.join(directory, `${name}.json`), stableJson(value)),
    text: (name, value) => writeFile(path.join(directory, name), value),
  };
}
