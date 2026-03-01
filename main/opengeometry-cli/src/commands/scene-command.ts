import { randomUUID } from "node:crypto";

import { CliUsageError } from "../cli/argv.js";
import {
  loadSessionState,
  requireSceneById,
  resolveTargetSceneId,
  saveSessionState
} from "../state/session-store.js";
import type { SceneState } from "../types.js";

export async function runSceneCommand(args: string[], cwd: string): Promise<string> {
  const subcommand = args[0];

  if (!subcommand) {
    throw new CliUsageError("Missing scene subcommand. Use: create|list|use|show");
  }

  const state = await loadSessionState({ cwd });

  switch (subcommand) {
    case "create": {
      const name = args.slice(1).join(" ").trim();
      if (!name) {
        throw new CliUsageError("Missing scene name. Usage: opengeometry scene create <name>");
      }

      const scene: SceneState = {
        id: randomUUID(),
        name,
        entities: []
      };

      state.scenes.push(scene);
      state.currentSceneId = scene.id;
      await saveSessionState(state, { cwd });

      return JSON.stringify(
        {
          id: scene.id,
          name: scene.name,
          current: true
        },
        null,
        2
      );
    }

    case "list": {
      const output = state.scenes.map((scene) => ({
        id: scene.id,
        name: scene.name,
        entityCount: scene.entities.length,
        current: scene.id === state.currentSceneId
      }));

      return JSON.stringify(output, null, 2);
    }

    case "use": {
      const sceneId = args[1];
      if (!sceneId) {
        throw new CliUsageError("Missing scene id. Usage: opengeometry scene use <sceneId>");
      }

      requireSceneById(state, sceneId);
      state.currentSceneId = sceneId;
      await saveSessionState(state, { cwd });

      return JSON.stringify({ currentSceneId: sceneId }, null, 2);
    }

    case "show": {
      const requestedId = args[1];
      const sceneId = resolveTargetSceneId(state, requestedId);
      const scene = requireSceneById(state, sceneId);

      return JSON.stringify(
        {
          id: scene.id,
          name: scene.name,
          entityCount: scene.entities.length,
          entities: scene.entities,
          current: scene.id === state.currentSceneId
        },
        null,
        2
      );
    }

    default:
      throw new CliUsageError(`Unknown scene subcommand '${subcommand}'`);
  }
}
