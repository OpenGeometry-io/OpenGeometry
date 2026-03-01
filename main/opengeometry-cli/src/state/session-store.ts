import { resolve } from "node:path";

import { readTextFileIfExists, writeTextFileAtomic } from "../io/json-file.js";
import {
  createEmptySessionState,
  type LineEntity,
  type SceneState,
  type SessionState
} from "../types.js";

const DEFAULT_SESSION_PATH = ".opengeometry/session.json";

interface SessionStoreOptions {
  cwd?: string;
  sessionPath?: string;
}

function isPoint3(value: unknown): value is { x: number; y: number; z: number } {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.x === "number" &&
    Number.isFinite(candidate.x) &&
    typeof candidate.y === "number" &&
    Number.isFinite(candidate.y) &&
    typeof candidate.z === "number" &&
    Number.isFinite(candidate.z)
  );
}

function isLineEntity(value: unknown): value is LineEntity {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return (
    candidate.kind === "line" &&
    typeof candidate.id === "string" &&
    isPoint3(candidate.start) &&
    isPoint3(candidate.end)
  );
}

function isSceneState(value: unknown): value is SceneState {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.id === "string" &&
    typeof candidate.name === "string" &&
    Array.isArray(candidate.entities) &&
    candidate.entities.every((entity) => isLineEntity(entity))
  );
}

function isSessionState(value: unknown): value is SessionState {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return (
    candidate.version === 1 &&
    (typeof candidate.currentSceneId === "string" || candidate.currentSceneId === null) &&
    Array.isArray(candidate.scenes) &&
    candidate.scenes.every((scene) => isSceneState(scene))
  );
}

export function resolveSessionPath(options?: SessionStoreOptions): string {
  const cwd = options?.cwd ?? process.cwd();
  const configuredPath = options?.sessionPath ?? DEFAULT_SESSION_PATH;
  return resolve(cwd, configuredPath);
}

export async function loadSessionState(options?: SessionStoreOptions): Promise<SessionState> {
  const sessionPath = resolveSessionPath(options);
  const content = await readTextFileIfExists(sessionPath);

  if (content === undefined || content.trim() === "") {
    return createEmptySessionState();
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch (error) {
    throw new Error(
      `Session file is not valid JSON: ${sessionPath}. ${(error as Error).message}`
    );
  }

  if (!isSessionState(parsed)) {
    throw new Error(
      `Session file has unsupported shape/version: ${sessionPath}. Delete it or run state migration.`
    );
  }

  return parsed;
}

export async function saveSessionState(
  state: SessionState,
  options?: SessionStoreOptions
): Promise<void> {
  const sessionPath = resolveSessionPath(options);
  const payload = JSON.stringify(state, null, 2);
  await writeTextFileAtomic(sessionPath, `${payload}\n`);
}

export function getSceneById(state: SessionState, sceneId: string): SceneState | undefined {
  return state.scenes.find((scene) => scene.id === sceneId);
}

export function requireSceneById(state: SessionState, sceneId: string): SceneState {
  const scene = getSceneById(state, sceneId);
  if (!scene) {
    throw new Error(`Scene '${sceneId}' does not exist in session state`);
  }

  return scene;
}

export function resolveTargetSceneId(state: SessionState, requestedSceneId?: string): string {
  if (requestedSceneId) {
    return requestedSceneId;
  }

  if (!state.currentSceneId) {
    throw new Error("No scene selected. Create one with `opengeometry scene create <name>`.");
  }

  return state.currentSceneId;
}
