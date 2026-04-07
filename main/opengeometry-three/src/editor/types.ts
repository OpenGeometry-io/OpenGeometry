import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import type { ObjectTransformation } from "../freeform/types";

/**
 * Numeric identifier used for faces, edges, and vertices in freeform topology APIs.
 */
export type TopologyId = number;

/**
 * Kernel edit permissions shared by global and feature-scoped freeform capabilities.
 */
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

/**
 * Global editing capabilities for a freeform geometry object.
 */
export interface FreeformEditCapabilities extends FreeformOperationCapabilities {
  editingMode: "freeform";
  entityType: "freeform";
  editFamily: "freeform";
  canEditConfig: false;
  canEditPlacement: false;
  canConvertToFreeform: false;
}

/**
 * Feature-scoped editing capabilities for a single face, edge, or vertex.
 */
export interface FreeformFeatureEditCapabilities
  extends FreeformOperationCapabilities {
  domain: "face" | "edge" | "vertex";
  topologyId: TopologyId;
}

/**
 * How a topology id changed after an edit.
 */
export type TopologyRemapStatus = "unchanged" | "split" | "merged" | "deleted";

/**
 * Maps an old topology id to the ids that replaced it.
 */
export interface TopologyRemapEntry {
  old_id: TopologyId;
  new_ids: TopologyId[];
  primary_id: TopologyId | null;
  status: TopologyRemapStatus;
}

/**
 * Newly created topology ids emitted by a freeform edit.
 */
export interface TopologyCreatedIds {
  faces: TopologyId[];
  edges: TopologyId[];
  vertices: TopologyId[];
}

/**
 * Topology remap payload returned by edits that change face/edge/vertex ids.
 */
export interface TopologyRemap {
  faces: TopologyRemapEntry[];
  edges: TopologyRemapEntry[];
  vertices: TopologyRemapEntry[];
  created_ids: TopologyCreatedIds;
}

/**
 * Diagnostic severities returned by freeform validity checks.
 */
export type DiagnosticSeverity = "info" | "warning" | "error";

/**
 * Individual diagnostic emitted by a freeform edit or validation pass.
 */
export interface FreeformDiagnostic {
  code: string;
  severity: DiagnosticSeverity;
  message: string;
  domain?: string;
  topology_id?: TopologyId;
}

/**
 * Validity summary returned after a freeform edit.
 */
export interface FreeformValidity {
  ok: boolean;
  healed?: boolean;
  diagnostics: FreeformDiagnostic[];
}

/**
 * Face metadata returned by the freeform editor.
 */
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

/**
 * Edge metadata returned by the freeform editor.
 */
export interface EdgeInfo {
  edge_id: TopologyId;
  curve_type: string;
  start_vertex_id: TopologyId;
  end_vertex_id: TopologyId;
  start: Vector3;
  end: Vector3;
  incident_face_ids: TopologyId[];
}

/**
 * Vertex metadata returned by the freeform editor.
 */
export interface VertexInfo {
  vertex_id: TopologyId;
  position: Vector3;
  edge_ids: TopologyId[];
  face_ids: TopologyId[];
}

/**
 * Triangulated face render data for topology overlays.
 */
export interface TopologyFaceRenderData {
  face_id: TopologyId;
  positions: number[];
  indices: number[];
}

/**
 * Edge polyline render data for topology overlays.
 */
export interface TopologyEdgeRenderData {
  edge_id: TopologyId;
  positions: number[];
}

/**
 * Vertex render data for topology overlays.
 */
export interface TopologyVertexRenderData {
  vertex_id: TopologyId;
  position: Vector3;
}

/**
 * Full renderable topology payload emitted by the freeform editor.
 */
export interface TopologyRenderData {
  faces: TopologyFaceRenderData[];
  edges: TopologyEdgeRenderData[];
  vertices: TopologyVertexRenderData[];
}

/**
 * Structured result payload returned by freeform edit operations.
 */
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

/**
 * Optional flags that control how much auxiliary data an edit returns.
 */
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

/**
 * High-level editing mode families exposed by public wrappers.
 */
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

/**
 * Shared placement structure for parametric wrappers.
 */
export interface ParametricPlacement {
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

/**
 * Base capability payload shared by parametric and freeform wrappers.
 */
export interface BaseEditCapabilities {
  editingMode: EditingMode;
  entityType: EntityType;
  editFamily: EditFamily;
  canEditConfig: boolean;
  canEditPlacement: boolean;
  canConvertToFreeform: boolean;
}

/**
 * Capability payload returned by parametric wrappers such as `Polygon` or `Cuboid`.
 */
export interface ParametricEditCapabilities extends BaseEditCapabilities {
  editingMode: "parametric";
  entityType: ParametricEntityType;
}
