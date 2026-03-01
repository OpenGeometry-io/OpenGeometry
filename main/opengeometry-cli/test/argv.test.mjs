import test from "node:test";
import assert from "node:assert/strict";

import {
  getOptionalStringOption,
  getRequiredStringOption,
  hasFlag,
  parseOptionTokens,
  parsePoint3
} from "../dist/cli/argv.js";

test("parseOptionTokens handles value and flag options", () => {
  const parsed = parseOptionTokens([
    "line",
    "--start",
    "0,1,2",
    "--pretty",
    "--scene=abc-123",
    "trailing"
  ]);

  assert.deepEqual(parsed.positionals, ["line", "trailing"]);
  assert.equal(getRequiredStringOption(parsed, "start"), "0,1,2");
  assert.equal(getOptionalStringOption(parsed, "scene"), "abc-123");
  assert.equal(hasFlag(parsed, "pretty"), true);
});

test("parsePoint3 parses x,y,z coordinates", () => {
  const point = parsePoint3("1.5, -2, 0", "--start");
  assert.deepEqual(point, { x: 1.5, y: -2, z: 0 });
});

test("parsePoint3 throws for invalid coordinate list", () => {
  assert.throws(() => parsePoint3("1,2", "--start"));
  assert.throws(() => parsePoint3("1,foo,3", "--start"));
});
