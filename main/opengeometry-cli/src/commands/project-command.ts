import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import {
  CliUsageError,
  getOptionalStringOption,
  hasFlag,
  parseOptionTokens
} from "../cli/argv.js";
import { buildKernelState } from "../kernel/kernel-runtime.js";
import {
  loadSessionState,
  requireSceneById,
  resolveTargetSceneId
} from "../state/session-store.js";

async function loadJsonPayload(pathValue: string, cwd: string, optionName: string): Promise<string> {
  const fullPath = resolve(cwd, pathValue);
  const raw = await readFile(fullPath, "utf8");

  try {
    return JSON.stringify(JSON.parse(raw));
  } catch (error) {
    throw new CliUsageError(
      `Invalid JSON in ${optionName} file '${fullPath}': ${(error as Error).message}`
    );
  }
}

export async function runProjectCommand(args: string[], cwd: string): Promise<string> {
  const subcommand = args[0];
  if (subcommand !== "2d") {
    throw new CliUsageError("Only `opengeometry project 2d` is supported in this slice");
  }

  const parsed = parseOptionTokens(args.slice(1));
  const requestedSceneId = getOptionalStringOption(parsed, "scene");
  const cameraJsonPath = getOptionalStringOption(parsed, "camera-json");
  const hlrJsonPath = getOptionalStringOption(parsed, "hlr-json");
  const pretty = hasFlag(parsed, "pretty");

  const state = await loadSessionState({ cwd });
  const sceneId = resolveTargetSceneId(state, requestedSceneId);
  requireSceneById(state, sceneId);

  const { manager, sceneIdMap, dispose } = buildKernelState(state);

  try {
    const runtimeSceneId = sceneIdMap.get(sceneId);
    if (!runtimeSceneId) {
      throw new Error(`Unable to resolve runtime scene mapping for scene '${sceneId}'`);
    }

    const cameraPayload = cameraJsonPath
      ? await loadJsonPayload(cameraJsonPath, cwd, "--camera-json")
      : "";
    const hlrPayload = hlrJsonPath
      ? await loadJsonPayload(hlrJsonPath, cwd, "--hlr-json")
      : undefined;

    if (pretty) {
      return manager.projectTo2DCameraPretty(runtimeSceneId, cameraPayload, hlrPayload ?? null);
    }

    return manager.projectTo2DCamera(runtimeSceneId, cameraPayload, hlrPayload ?? null);
  } finally {
    dispose();
  }
}
