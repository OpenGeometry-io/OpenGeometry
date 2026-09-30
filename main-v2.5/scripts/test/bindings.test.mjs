import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, OGWorldGraph, OGTessellator } from '../../opengeometry/pkg/opengeometry.js';

initSync({ module: readFileSync(new URL('../../opengeometry/pkg/opengeometry_bg.wasm', import.meta.url)) });
const ACCURACY = { geometric: 1e-8, intersection: 1e-9, tessellation: 0.01, exchange: 1e-6 };
const GRAPH = new OGWorldGraph(JSON.stringify(ACCURACY));
const ENCODE = JSON.stringify;
const CAMEL_CASE_KEY = /^[a-z][A-Za-z0-9]*$/;
const CAMEL_CASE_KEYS = (value) => {
  if (Array.isArray(value)) {
    value.forEach(CAMEL_CASE_KEYS);
  } else if (value !== null && typeof value === 'object') {
    for (const [key, inner] of Object.entries(value)) {
      assert.match(key, CAMEL_CASE_KEY);
      CAMEL_CASE_KEYS(inner);
    }
  }
};
const UNIT_CUBE = { kind: 'Cuboid', width: 1, height: 1, depth: 1 };
const CUBE = JSON.parse(GRAPH.createPrimitive(ENCODE(UNIT_CUBE), ENCODE({ og_id: 'cube' })));
assert.equal(CUBE.ogId, 'cube');
CAMEL_CASE_KEYS(CUBE);
const INFO = JSON.parse(GRAPH.node('cube'));
CAMEL_CASE_KEYS(INFO);
assert.equal(INFO.bodyType, 'Solid');
assert.equal(JSON.parse(GRAPH.nodeByHandle(INFO.handle, INFO.generation)).ogId, 'cube');
GRAPH.transform('cube', ENCODE({ kind: 'Place', origin: [0, 0, 0], xDirection: [1, 0, 0] }));
assert.throws(
  () => GRAPH.transform('cube', '{"kind":"Place","x_direction":[1,0,0]}'),
  (error) => JSON.parse(String(error)).code === 'InvalidParameter',
);
const PLACEMENT = JSON.parse(GRAPH.placement('cube'));
CAMEL_CASE_KEYS(PLACEMENT);
assert.deepEqual(Object.keys(PLACEMENT), ['origin', 'xDirection', 'normal', 'scale']);
assert.throws(
  () => GRAPH.createPrimitive(ENCODE({ kind: 'Cuboid', width: 1, height: 1, depth: 1, extra: 1 }), '{}'),
  (error) => {
    const parsed = JSON.parse(String(error));
    CAMEL_CASE_KEYS(parsed);
    return parsed.code === 'InvalidParameter';
  },
);
assert.throws(
  () => GRAPH.createPrimitive(ENCODE({ kind: 'Polyline', points: [[0, 0, 0], [1, 0, 0]], closed: false }), '{}'),
  (error) => JSON.parse(String(error)).code === 'InvalidParameter',
);

const SNAPSHOT = GRAPH.snapshot(INFO.shapeId);
assert(SNAPSHOT instanceof Uint8Array);
const TESSELLATOR = new OGTessellator();
const SLOT = TESSELLATOR.load(SNAPSHOT);
const DIRECT = GRAPH.buffers(INFO.shapeId, 0.01, 1000);
const LOADED = TESSELLATOR.buffers(SLOT, 0.01, 1000);
assert.deepEqual(DIRECT.positions, LOADED.positions);
assert.deepEqual(DIRECT.faceRanges, LOADED.faceRanges);
assert.equal(DIRECT.triangles, 12);
assert.equal(TESSELLATOR.drop(SLOT), true);

const MARK = GRAPH.mark();
GRAPH.transform('cube', ENCODE({ kind: 'Translate', offset: [2, 0, 0] }));
assert(Math.abs(JSON.parse(GRAPH.bounds('cube'))[0] - 1.5) < 1e-6);
GRAPH.rollback(MARK);
GRAPH.release(MARK);
assert(Math.abs(JSON.parse(GRAPH.bounds('cube'))[0] + 0.5) < 1e-6);
const CHANGES = GRAPH.changesSince(0n);
assert(CHANGES.matrices instanceof Float64Array);
assert.equal(CHANGES.matrices.length, 16);
const CHANGES_JSON = JSON.parse(CHANGES.changesJson);
CAMEL_CASE_KEYS(CHANGES_JSON);
assert([...CHANGES_JSON.added, ...CHANGES_JSON.changed].some((entry) => entry.ogId === 'cube'));
CAMEL_CASE_KEYS(JSON.parse(GRAPH.markStats()));

const COPY = JSON.parse(GRAPH.instance('cube', '{}'));
assert.equal(GRAPH.instanceCount('cube'), 2);
GRAPH.makeUnique(COPY.ogId);
assert.equal(GRAPH.instanceCount('cube'), 1);
const EXPORTED = GRAPH.exportStep(ENCODE(['cube']), '{}');
assert(EXPORTED.text.includes('MANIFOLD_SOLID_BREP'));
assert.equal(JSON.parse(EXPORTED.reportJson).products, 1);
CAMEL_CASE_KEYS(JSON.parse(EXPORTED.reportJson));
GRAPH.dispose('cube');
assert.throws(
  () => GRAPH.nodeByHandle(INFO.handle, INFO.generation),
  (error) => JSON.parse(String(error)).code === 'Disposed',
);
TESSELLATOR.free();
GRAPH.free();
console.log('WASM bindings passed');
