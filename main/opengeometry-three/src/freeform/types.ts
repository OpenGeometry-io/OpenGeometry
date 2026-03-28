import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

export interface FreeformPlacementInput {
  anchor?: Vector3;
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

export interface ObjectTransformation {
  anchor: Vector3;
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

export interface CreateFreeformGeometryOptions {
  id?: string;
  placement?: FreeformPlacementInput | ObjectTransformation;
}

export type FreeformSource =
  | string
  | Record<string, unknown>
  | {
      getLocalBrepSerialized?: () => string;
      getLocalBrepData?: () => unknown;
      getBrepSerialized?: () => string;
      getBrepData?: () => unknown;
      getBrep?: () => unknown;
    };
