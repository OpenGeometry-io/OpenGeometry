import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

/**
 * Placement values accepted when constructing freeform geometry wrappers.
 */
export interface FreeformPlacementInput {
  anchor?: Vector3;
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

/**
 * Fully normalized placement state used by freeform editing APIs.
 */
export interface ObjectTransformation {
  anchor: Vector3;
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

/**
 * Options for building a `FreeformGeometry` wrapper from a BRep-like source.
 */
export interface CreateFreeformGeometryOptions {
  id?: string;
  placement?: FreeformPlacementInput | ObjectTransformation;
}

/**
 * Accepted sources for creating a freeform wrapper.
 */
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
