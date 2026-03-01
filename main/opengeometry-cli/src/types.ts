export interface Point3 {
  x: number;
  y: number;
  z: number;
}

export interface LineEntity {
  kind: "line";
  id: string;
  start: Point3;
  end: Point3;
}

export type SceneEntity = LineEntity;

export interface SceneState {
  id: string;
  name: string;
  entities: SceneEntity[];
}

export interface SessionState {
  version: 1;
  currentSceneId: string | null;
  scenes: SceneState[];
}

export const SESSION_VERSION = 1;

export function createEmptySessionState(): SessionState {
  return {
    version: SESSION_VERSION,
    currentSceneId: null,
    scenes: []
  };
}
