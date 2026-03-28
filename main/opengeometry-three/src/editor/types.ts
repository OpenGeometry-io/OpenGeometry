import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import type { ObjectTransformation } from "../freeform/types";

export type TopologyId = number;

export interface FreeformOperationCapabilities {
  canPushPullFace: boolean;
  canMoveFace: boolean;
  canExtrudeFace: boolean;
  canCutFace: boolean;
  canMoveEdge: boolean;
  canMoveVertex: boolean;
  canInsertVertexOnEdge: boolean;
  canRemoveVertex: boolean;
  canSplitEdge: boolean;
  canLoopCut: boolean;
  reasons?: string[];
}

export interface FreeformEditCapabilities extends FreeformOperationCapabilities {
  editingMode: "freeform";
  entityType: "freeform";
  editFamily: "freeform";
  canEditConfig: false;
  canEditPlacement: false;
  canConvertToFreeform: false;
}

export interface FreeformFeatureEditCapabilities
  extends FreeformOperationCapabilities {
  domain: "face" | "edge" | "vertex";
  topologyId: TopologyId;
}

export type TopologyRemapStatus = "unchanged" | "split" | "merged" | "deleted";

export interface TopologyRemapEntry {
  old_id: TopologyId;
  new_ids: TopologyId[];
  primary_id: TopologyId | null;
  status: TopologyRemapStatus;
}

export interface TopologyCreatedIds {
  faces: TopologyId[];
  edges: TopologyId[];
  vertices: TopologyId[];
}

export interface TopologyRemap {
  faces: TopologyRemapEntry[];
  edges: TopologyRemapEntry[];
  vertices: TopologyRemapEntry[];
  created_ids: TopologyCreatedIds;
}

export type DiagnosticSeverity = "info" | "warning" | "error";

export interface FreeformDiagnostic {
  code: string;
  severity: DiagnosticSeverity;
  message: string;
  domain?: string;
  topology_id?: TopologyId;
}

export interface FreeformValidity {
  ok: boolean;
  healed?: boolean;
  diagnostics: FreeformDiagnostic[];
}

export interface FaceInfo {
  face_id: TopologyId;
  centroid: Vector3;
  normal: Vector3;
  surface_type: string;
  loop_ids: TopologyId[];
  edge_ids: TopologyId[];
  vertex_ids: TopologyId[];
  adjacent_face_ids: TopologyId[];
}

export interface EdgeInfo {
  edge_id: TopologyId;
  curve_type: string;
  start_vertex_id: TopologyId;
  end_vertex_id: TopologyId;
  start: Vector3;
  end: Vector3;
  incident_face_ids: TopologyId[];
}

export interface VertexInfo {
  vertex_id: TopologyId;
  position: Vector3;
  edge_ids: TopologyId[];
  face_ids: TopologyId[];
}

export interface TopologyFaceRenderData {
  face_id: TopologyId;
  positions: number[];
  indices: number[];
}

export interface TopologyEdgeRenderData {
  edge_id: TopologyId;
  positions: number[];
}

export interface TopologyVertexRenderData {
  vertex_id: TopologyId;
  position: Vector3;
}

export interface TopologyRenderData {
  faces: TopologyFaceRenderData[];
  edges: TopologyEdgeRenderData[];
  vertices: TopologyVertexRenderData[];
}

export interface FreeformEditResult {
  entity_id: string;
  brep_serialized?: string;
  local_brep_serialized?: string;
  geometry_serialized?: string;
  outline_geometry_serialized?: string;
  topology_changed: boolean;
  topology_remap?: TopologyRemap;
  changed_faces?: TopologyId[];
  changed_edges?: TopologyId[];
  changed_vertices?: TopologyId[];
  validity: FreeformValidity;
  placement: ObjectTransformation;
}

export interface EditOperationOptions {
  includeBrepSerialized?: boolean;
  includeLocalBrepSerialized?: boolean;
  includeGeometrySerialized?: boolean;
  includeOutlineGeometrySerialized?: boolean;
  includeTopologyRemap?: boolean;
  includeDeltas?: boolean;
  constraintAxis?: Vector3;
  constraintPlaneNormal?: Vector3;
  preserveCoplanarity?: boolean;
  constraintFrame?: "local" | "world";
  openSurfaceMode?: boolean;
}

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
