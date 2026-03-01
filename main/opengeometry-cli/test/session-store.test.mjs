import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

import {
  loadSessionState,
  resolveSessionPath,
  saveSessionState
} from "../dist/state/session-store.js";
import { createEmptySessionState } from "../dist/types.js";

test("loadSessionState returns empty state when file does not exist", async () => {
  const cwd = await mkdtemp(join(tmpdir(), "og-cli-"));
  const state = await loadSessionState({ cwd });
  assert.deepEqual(state, createEmptySessionState());
});

test("saveSessionState writes JSON session file", async () => {
  const cwd = await mkdtemp(join(tmpdir(), "og-cli-"));
  const state = createEmptySessionState();
  state.currentSceneId = "scene-1";
  state.scenes.push({
    id: "scene-1",
    name: "Sample",
    entities: []
  });

  await saveSessionState(state, { cwd });
  const sessionPath = resolveSessionPath({ cwd });
  const written = await readFile(sessionPath, "utf8");

  assert.equal(JSON.parse(written).currentSceneId, "scene-1");
});
