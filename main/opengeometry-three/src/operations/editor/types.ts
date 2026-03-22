import { Vector3 } from "../../../../opengeometry/pkg/opengeometry";
export type {
  CreateFreeformGeometryOptions,
  EdgeInfo,
  EditOperationOptions,
  FaceInfo,
  FreeformDiagnostic,
  FreeformEditCapabilities,
  FreeformEditResult,
  FreeformFeatureEditCapabilities,
  FreeformOperationCapabilities,
  FreeformSource,
  FreeformValidity,
  ObjectTransformation,
  TopologyCreatedIds,
  TopologyEdgeRenderData,
  TopologyFaceRenderData,
  TopologyId,
  TopologyRemap,
  TopologyRemapEntry,
  TopologyRemapStatus,
  TopologyRenderData,
  TopologyVertexRenderData,
  VertexInfo,
} from "../../freeform/types";

export type EditingMode = "parametric" | "freeform";
export type ParametricEntityType =
  | "line"
  | "polyline"
  | "arc"
  | "curve"
  | "rectangle"
  | "polygon"
  | "cuboid"
  | "cylinder"
  | "sphere"
  | "wedge"
  | "opening"
  | "sweep";
export type EntityType = ParametricEntityType | "freeform";
export type EditFamily =
  | "profile"
  | "curve"
  | "box"
  | "radial"
  | "wedge"
  | "sweep"
  | "freeform";
export type ParametricEditFamily = Exclude<EditFamily, "freeform">;

export interface ParametricPlacement {
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

export interface BaseEditCapabilities {
  editingMode: EditingMode;
  entityType: EntityType;
  editFamily: EditFamily;
  canEditConfig: boolean;
  canEditPlacement: boolean;
  canConvertToFreeform: boolean;
}

export interface ParametricEditCapabilities extends BaseEditCapabilities {
  editingMode: "parametric";
  entityType: ParametricEntityType;
}
