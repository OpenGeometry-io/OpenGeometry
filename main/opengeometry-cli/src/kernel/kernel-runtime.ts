import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  OGLine,
  OGSceneManager,
  Vector3,
  initSync
} from "../../../opengeometry/pkg/opengeometry.js";

import type { SessionState } from "../types.js";

let initialized = false;

function ensureKernelInitialized(): void {
  if (initialized) {
    return;
  }

  const thisFile = fileURLToPath(import.meta.url);
  const thisDir = dirname(thisFile);
  const wasmPath = resolve(thisDir, "../../../opengeometry/pkg/opengeometry_bg.wasm");
  const wasmBytes = readFileSync(wasmPath);
  initSync({ module: wasmBytes });
  initialized = true;
}

export interface BuiltKernelState {
  manager: OGSceneManager;
  sceneIdMap: Map<string, string>;
  dispose: () => void;
}

export function buildKernelState(session: SessionState): BuiltKernelState {
  ensureKernelInitialized();

  const manager = new OGSceneManager();
  const sceneIdMap = new Map<string, string>();

  for (const scene of session.scenes) {
    const runtimeSceneId = manager.createScene(scene.name);
    sceneIdMap.set(scene.id, runtimeSceneId);
  }

  for (const scene of session.scenes) {
    const runtimeSceneId = sceneIdMap.get(scene.id);
    if (!runtimeSceneId) {
      throw new Error(`Runtime scene mapping missing for scene '${scene.id}'`);
    }

    for (const entity of scene.entities) {
      if (entity.kind !== "line") {
        continue;
      }

      const line = new OGLine(entity.id);

      try {
        line.set_config(
          new Vector3(entity.start.x, entity.start.y, entity.start.z),
          new Vector3(entity.end.x, entity.end.y, entity.end.z)
        );
        line.generate_geometry();
        manager.addLineToScene(runtimeSceneId, entity.id, line);
      } finally {
        line.free();
      }
    }
  }

  if (session.currentSceneId) {
    const runtimeCurrent = sceneIdMap.get(session.currentSceneId);
    if (runtimeCurrent) {
      manager.setCurrentScene(runtimeCurrent);
    }
  }

  return {
    manager,
    sceneIdMap,
    dispose: () => {
      manager.free();
    }
  };
}
