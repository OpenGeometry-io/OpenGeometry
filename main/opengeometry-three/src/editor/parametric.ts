import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import type {
  ParametricEditCapabilities,
  ParametricEditFamily,
  ParametricEntityType,
  ParametricPlacement,
} from "./types";
import type { ObjectTransformation } from "../freeform/types";

function cloneVector(
  vector: Vector3 | undefined,
  fallback: [number, number, number]
): Vector3 {
  return vector?.clone() ?? new Vector3(...fallback);
}

/**
 * Returns the canonical capability payload exposed by parametric wrappers.
 */
export function createParametricEditCapabilities(
  entityType: ParametricEntityType,
  editFamily: ParametricEditFamily
): ParametricEditCapabilities {
  return {
    editingMode: "parametric",
    entityType,
    editFamily,
    canEditConfig: true,
    canEditPlacement: true,
    canConvertToFreeform: true,
  };
}

/**
 * Normalizes optional placement fields into concrete vectors.
 */
export function clonePlacement(
  placement: Partial<ParametricPlacement>
): ParametricPlacement {
  return {
    translation: cloneVector(placement.translation, [0, 0, 0]),
    rotation: cloneVector(placement.rotation, [0, 0, 0]),
    scale: cloneVector(placement.scale, [1, 1, 1]),
  };
}

/**
 * Converts a parametric placement payload into the freeform/object transform
 * structure expected by BRep-backed helpers.
 */
export function toObjectTransformation(
  placement: ParametricPlacement | ObjectTransformation
): ObjectTransformation {
  if ("anchor" in placement) {
    return {
      anchor: cloneVector(placement.anchor, [0, 0, 0]),
      translation: cloneVector(placement.translation, [0, 0, 0]),
      rotation: cloneVector(placement.rotation, [0, 0, 0]),
      scale: cloneVector(placement.scale, [1, 1, 1]),
    };
  }

  return {
    anchor: new Vector3(0, 0, 0),
    translation: cloneVector(placement.translation, [0, 0, 0]),
    rotation: cloneVector(placement.rotation, [0, 0, 0]),
    scale: cloneVector(placement.scale, [1, 1, 1]),
  };
}
