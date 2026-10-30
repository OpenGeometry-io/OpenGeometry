import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  OpenGeometry, Sheet, Solid, SystemAssembly, Wire, OG_OPERATION_SUBTRACT, OG_OPERATION_SWEEP,
  OG_PRIMITIVE_CUBOID, OG_PRIMITIVE_POLYLINE, OG_PRIMITIVE_RECTANGLE, OG_TRANSFORM_TRANSLATE,
} from '../../../dist/index.js';
import { graph } from '../../../dist/testing.js';
import { boot, cuboid, failsWith } from './support.mjs';

function floorkitContract(body) {
  if (body.schema_version !== 2 || body.length_unit !== 'metre' || body.up_axis !== 'Z'
    || body.coordinate_space !== 'world') {
    throw new Error('Invalid Rust-prepared analytic exchange contract');
  }
  return body;
}

function points(body) {
  return body.entities
    .filter((entity) => entity.entity_type === 'IfcCartesianPoint')
    .map((entity) => entity.attributes[0].values.map((value) => value.value))
    .filter((point) => point.length === 3);
}

test("exportIfcBody passes floorkit's exchange contract check", async () => {
  await boot();
  const body = cuboid('ifc-contract');
  const exported = floorkitContract(OpenGeometry.exportIfcBody(body));
  assert.equal(exported.representation_type, 'AdvancedBrep');
  assert.equal(exported.solids.length, 1);
  assert.equal(exported.faces.length, 6);
  assert(exported.entities.some((entity) => entity.entity_type === 'IfcAdvancedBrep'));
  assert.deepEqual(exported.entities.map((entity) => entity.id), exported.entities.map((_, index) => index + 1));
  body.dispose();
});

test('exportIfcBody places the body in the world with Z up', async () => {
  await boot();
  const body = cuboid('ifc-moved');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [10, 3, 0] });
  const placed = points(OpenGeometry.exportIfcBody(body));
  assert(placed.some(([x]) => x >= 10.4));
  assert(placed.some(([, , z]) => z >= 3.9));
  assert(placed.every(([, y]) => Math.abs(y) <= 0.5 + 1e-9));
  body.dispose();
});

test('a body with a cavity exports as CSG with an IfcBooleanResult difference', async () => {
  await boot();
  const host = new Solid(OG_PRIMITIVE_CUBOID, { width: 3, height: 3, depth: 3 }, { ogId: 'ifc-host' });
  const tool = new Solid(OG_PRIMITIVE_CUBOID, { width: 1, height: 1, depth: 1 }, { ogId: 'ifc-tool' });
  tool.transform(OG_TRANSFORM_TRANSLATE, { offset: [0, 1, 0] });
  host.operate(OG_OPERATION_SUBTRACT, { tools: [tool] });
  const exported = floorkitContract(OpenGeometry.exportIfcBody(host));
  assert.equal(exported.representation_type, 'CSG');
  const result = exported.entities.find((entity) => entity.entity_type === 'IfcBooleanResult');
  assert.deepEqual(result?.attributes[0], { kind: 'Enum', value: 'DIFFERENCE' });
  assert.equal(exported.solids.length, 1);
  host.dispose();
  tool.dispose();
});

test('exportIfcBody refuses a Sheet, a Wire and an assembly with InvalidOperand', async () => {
  await boot();
  const profile = new Wire(OG_PRIMITIVE_RECTANGLE, { width: 1, breadth: 1 }, { ogId: 'ifc-profile' });
  const path = new Wire(OG_PRIMITIVE_POLYLINE, { points: [[0, 0, 0], [0, 2, 0]] }, { ogId: 'ifc-path' });
  const sheet = new Sheet(OG_OPERATION_SWEEP, { profile, path, caps: false }, { ogId: 'ifc-sheet' });
  const assembly = new SystemAssembly({ ogId: 'ifc-assembly' });
  for (const node of [sheet, profile, assembly]) {
    assert.throws(() => OpenGeometry.exportIfcBody(node), failsWith('InvalidOperand'));
  }
  assembly.dispose();
  sheet.dispose();
  path.dispose();
  profile.dispose();
});

test('exportIfcBody of a disposed body throws Disposed', async () => {
  await boot();
  const body = cuboid('ifc-disposed');
  body.dispose();
  assert.throws(() => OpenGeometry.exportIfcBody(body), failsWith('Disposed'));
});

test('exportIfcBody leaves revision, changes, mark stats and getBrep unchanged', async () => {
  await boot();
  const body = cuboid('ifc-state');
  body.transform(OG_TRANSFORM_TRANSLATE, { offset: [1, 2, 3] });
  const revision = graph().revision();
  const state = () => JSON.stringify([
    String(graph().revision()), graph().changesSince(revision).changesJson, OpenGeometry.markStats(), body.getBrep(),
  ]);
  const before = state();
  OpenGeometry.exportIfcBody(body);
  assert.equal(state(), before);
  body.dispose();
});
