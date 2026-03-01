import { randomUUID } from "node:crypto";

import {
  CliUsageError,
  getOptionalStringOption,
  getRequiredStringOption,
  parseOptionTokens,
  parsePoint3
} from "../cli/argv.js";
import {
  loadSessionState,
  requireSceneById,
  resolveTargetSceneId,
  saveSessionState
} from "../state/session-store.js";
import type { LineEntity } from "../types.js";

export async function runAddCommand(args: string[], cwd: string): Promise<string> {
  const subcommand = args[0];

  if (subcommand !== "line") {
    throw new CliUsageError("Only `opengeometry add line` is supported in this slice");
  }

  const parsed = parseOptionTokens(args.slice(1));
  const startValue = getRequiredStringOption(parsed, "start");
  const endValue = getRequiredStringOption(parsed, "end");
  const requestedEntityId = getOptionalStringOption(parsed, "id");
  const requestedSceneId = getOptionalStringOption(parsed, "scene");

  const state = await loadSessionState({ cwd });
  const sceneId = resolveTargetSceneId(state, requestedSceneId);
  const scene = requireSceneById(state, sceneId);

  const lineEntity: LineEntity = {
    kind: "line",
    id: requestedEntityId ?? randomUUID(),
    start: parsePoint3(startValue, "--start"),
    end: parsePoint3(endValue, "--end")
  };

  const existingIndex = scene.entities.findIndex((entity) => entity.id === lineEntity.id);
  if (existingIndex >= 0) {
    scene.entities[existingIndex] = lineEntity;
  } else {
    scene.entities.push(lineEntity);
  }

  if (state.currentSceneId === null) {
    state.currentSceneId = scene.id;
  }

  await saveSessionState(state, { cwd });

  return JSON.stringify(
    {
      sceneId: scene.id,
      entityId: lineEntity.id,
      entityKind: lineEntity.kind
    },
    null,
    2
  );
}
