import {
  OGFreeformGeometry,
  Vector3,
} from "../../../opengeometry/pkg/opengeometry";

import type {
  CreateFreeformGeometryOptions,
  EdgeInfo,
  EditOperationOptions,
  FaceInfo,
  FreeformEditCapabilities,
  FreeformEditResult,
  FreeformFeatureEditCapabilities,
  FreeformSource,
  ObjectTransformation,
  TopologyId,
  TopologyRenderData,
  VertexInfo,
} from "./types";

type RawFreeformOperationCapabilities = {
  can_push_pull_face: boolean;
  can_move_face: boolean;
  can_extrude_face: boolean;
  can_cut_face: boolean;
  can_move_edge: boolean;
  can_move_vertex: boolean;
  can_insert_vertex_on_edge: boolean;
  can_remove_vertex: boolean;
  can_split_edge: boolean;
  can_loop_cut: boolean;
  reasons?: string[];
};

type RawFreeformFeatureEditCapabilities = RawFreeformOperationCapabilities & {
  domain: "face" | "edge" | "vertex";
  topology_id: TopologyId;
};

function parseJson<T>(payload: string): T {
  return JSON.parse(payload) as T;
}

function toVector3(value: { x: number; y: number; z: number }): Vector3 {
  return new Vector3(value.x, value.y, value.z);
}

function toPlainVector3(value: { x: number; y: number; z: number }) {
  return {
    x: value.x,
    y: value.y,
    z: value.z,
  };
}

function cloneVector(
  vector: Vector3 | undefined,
  fallback: [number, number, number]
): Vector3 {
  return vector?.clone() ?? new Vector3(...fallback);
}

function normalizePlacement(
  placement: ObjectTransformation
): ObjectTransformation {
  return {
    anchor: toVector3(placement.anchor),
    translation: toVector3(placement.translation),
    rotation: toVector3(placement.rotation),
    scale: toVector3(placement.scale),
  };
}

function toObjectTransformation(
  placement: NonNullable<CreateFreeformGeometryOptions["placement"]>
): ObjectTransformation {
  return {
    anchor: cloneVector(placement.anchor, [0, 0, 0]),
    translation: cloneVector(placement.translation, [0, 0, 0]),
    rotation: cloneVector(placement.rotation, [0, 0, 0]),
    scale: cloneVector(placement.scale, [1, 1, 1]),
  };
}

function toPlainObjectTransformation(transform: ObjectTransformation) {
  return {
    anchor: toPlainVector3(transform.anchor),
    translation: toPlainVector3(transform.translation),
    rotation: toPlainVector3(transform.rotation),
    scale: toPlainVector3(transform.scale),
  };
}

function normalizeFaceInfo(face: FaceInfo): FaceInfo {
  return {
    ...face,
    centroid: toVector3(face.centroid),
    normal: toVector3(face.normal),
  };
}

function normalizeEdgeInfo(edge: EdgeInfo): EdgeInfo {
  return {
    ...edge,
    start: toVector3(edge.start),
    end: toVector3(edge.end),
  };
}

function normalizeVertexInfo(vertex: VertexInfo): VertexInfo {
  return {
    ...vertex,
    position: toVector3(vertex.position),
  };
}

function normalizeFeatureCapabilities(
  raw: RawFreeformFeatureEditCapabilities
): FreeformFeatureEditCapabilities {
  return {
    domain: raw.domain,
    topologyId: raw.topology_id,
    canPushPullFace: raw.can_push_pull_face,
    canMoveFace: raw.can_move_face,
    canExtrudeFace: raw.can_extrude_face,
    canCutFace: raw.can_cut_face,
    canMoveEdge: raw.can_move_edge,
    canMoveVertex: raw.can_move_vertex,
    canInsertVertexOnEdge: raw.can_insert_vertex_on_edge,
    canRemoveVertex: raw.can_remove_vertex,
    canSplitEdge: raw.can_split_edge,
    canLoopCut: raw.can_loop_cut,
    reasons: raw.reasons,
  };
}

function normalizeFreeformCapabilities(
  raw: RawFreeformOperationCapabilities
): FreeformEditCapabilities {
  return {
    editingMode: "freeform",
    entityType: "freeform",
    editFamily: "freeform",
    canEditConfig: false,
    canEditPlacement: true,
    canConvertToFreeform: false,
    canPushPullFace: raw.can_push_pull_face,
    canMoveFace: raw.can_move_face,
    canExtrudeFace: raw.can_extrude_face,
    canCutFace: raw.can_cut_face,
    canMoveEdge: raw.can_move_edge,
    canMoveVertex: raw.can_move_vertex,
    canInsertVertexOnEdge: raw.can_insert_vertex_on_edge,
    canRemoveVertex: raw.can_remove_vertex,
    canSplitEdge: raw.can_split_edge,
    canLoopCut: raw.can_loop_cut,
    reasons: raw.reasons,
  };
}

function normalizeFreeformSerialized(source: FreeformSource): string {
  if (typeof source === "string") {
    return source;
  }

  if (typeof source.getLocalBrepSerialized === "function") {
    return source.getLocalBrepSerialized();
  }

  if (typeof source.getLocalBrepData === "function") {
    return JSON.stringify(source.getLocalBrepData());
  }

  if (typeof source.getBrepSerialized === "function") {
    return source.getBrepSerialized();
  }

  if (typeof source.getBrepData === "function") {
    return JSON.stringify(source.getBrepData());
  }

  if (typeof source.getBrep === "function") {
    return JSON.stringify(source.getBrep());
  }

  return JSON.stringify(source);
}

function parseEditResult(payload: string): FreeformEditResult {
  const result = parseJson<FreeformEditResult>(payload);
  return {
    ...result,
    placement: normalizePlacement(result.placement),
  };
}

function serializeOptions(options?: EditOperationOptions): string | undefined {
  if (!options) {
    return undefined;
  }

  return JSON.stringify({
    ...options,
    constraintAxis: options.constraintAxis
      ? toPlainVector3(options.constraintAxis)
      : undefined,
    constraintPlaneNormal: options.constraintPlaneNormal
      ? toPlainVector3(options.constraintPlaneNormal)
      : undefined,
  });
}

export class FreeformGeometry {
  private readonly geometry: OGFreeformGeometry;

  constructor(geometry: OGFreeformGeometry) {
    this.geometry = geometry;
  }

  getId(): string {
    return this.geometry.id;
  }

  getBrepSerialized(): string {
    return this.geometry.getBrepSerialized();
  }

  getLocalBrepSerialized(): string {
    return this.geometry.getLocalBrepSerialized();
  }

  getGeometrySerialized(): string {
    return this.geometry.getGeometrySerialized();
  }

  getOutlineGeometrySerialized(): string {
    return this.geometry.getOutlineGeometrySerialized();
  }

  getPlacement(): ObjectTransformation {
    return normalizePlacement(
      parseJson<ObjectTransformation>(this.geometry.getPlacementSerialized())
    );
  }

  setPlacement(transform: ObjectTransformation): void {
    this.geometry.setPlacementSerialized(
      JSON.stringify(toPlainObjectTransformation(transform))
    );
  }

  setTransform(translation: Vector3, rotation: Vector3, scale: Vector3): void {
    this.geometry.setTransform(translation, rotation, scale);
  }

  setTranslation(translation: Vector3): void {
    this.geometry.setTranslation(translation);
  }

  setRotation(rotation: Vector3): void {
    this.geometry.setRotation(rotation);
  }

  setScale(scale: Vector3): void {
    this.geometry.setScale(scale);
  }

  setAnchor(anchor: Vector3): void {
    this.geometry.setAnchor(anchor);
  }

  getTopologyRenderData(): TopologyRenderData {
    return parseJson<TopologyRenderData>(this.geometry.getTopologyRenderData());
  }

  getFaceInfo(faceId: TopologyId): FaceInfo {
    return normalizeFaceInfo(parseJson<FaceInfo>(this.geometry.getFaceInfo(faceId)));
  }

  getEdgeInfo(edgeId: TopologyId): EdgeInfo {
    return normalizeEdgeInfo(parseJson<EdgeInfo>(this.geometry.getEdgeInfo(edgeId)));
  }

  getVertexInfo(vertexId: TopologyId): VertexInfo {
    return normalizeVertexInfo(
      parseJson<VertexInfo>(this.geometry.getVertexInfo(vertexId))
    );
  }

  getEditCapabilities(): FreeformEditCapabilities {
    return normalizeFreeformCapabilities(
      parseJson<RawFreeformOperationCapabilities>(
        this.geometry.getEditCapabilities()
      )
    );
  }

  getFaceEditCapabilities(faceId: TopologyId): FreeformFeatureEditCapabilities {
    return normalizeFeatureCapabilities(
      parseJson<RawFreeformFeatureEditCapabilities>(
        this.geometry.getFaceEditCapabilities(faceId)
      )
    );
  }

  getEdgeEditCapabilities(edgeId: TopologyId): FreeformFeatureEditCapabilities {
    return normalizeFeatureCapabilities(
      parseJson<RawFreeformFeatureEditCapabilities>(
        this.geometry.getEdgeEditCapabilities(edgeId)
      )
    );
  }

  getVertexEditCapabilities(vertexId: TopologyId): FreeformFeatureEditCapabilities {
    return normalizeFeatureCapabilities(
      parseJson<RawFreeformFeatureEditCapabilities>(
        this.geometry.getVertexEditCapabilities(vertexId)
      )
    );
  }

  pushPullFace(
    faceId: TopologyId,
    distance: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.pushPullFace(faceId, distance, serializeOptions(options))
    );
  }

  extrudeFace(
    faceId: TopologyId,
    distance: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.extrudeFace(faceId, distance, serializeOptions(options))
    );
  }

  cutFace(
    faceId: TopologyId,
    startEdgeId: TopologyId,
    startT: number,
    endEdgeId: TopologyId,
    endT: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.cutFace(
        faceId,
        startEdgeId,
        startT,
        endEdgeId,
        endT,
        serializeOptions(options)
      )
    );
  }

  moveFace(
    faceId: TopologyId,
    translation: Vector3,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.moveFace(faceId, translation, serializeOptions(options))
    );
  }

  moveEdge(
    edgeId: TopologyId,
    translation: Vector3,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.moveEdge(edgeId, translation, serializeOptions(options))
    );
  }

  moveVertex(
    vertexId: TopologyId,
    translation: Vector3,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.moveVertex(vertexId, translation, serializeOptions(options))
    );
  }

  insertVertexOnEdge(
    edgeId: TopologyId,
    t: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.insertVertexOnEdge(edgeId, t, serializeOptions(options))
    );
  }

  splitEdge(
    edgeId: TopologyId,
    t: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.splitEdge(edgeId, t, serializeOptions(options))
    );
  }

  loopCut(
    edgeId: TopologyId,
    t: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.loopCut(edgeId, t, serializeOptions(options))
    );
  }

  removeVertex(
    vertexId: TopologyId,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.geometry.removeVertex(vertexId, serializeOptions(options))
    );
  }
}

export function createFreeformGeometry(
  source: FreeformSource,
  options?: CreateFreeformGeometryOptions
): FreeformGeometry {
  const id = options?.id ?? `freeform-${Date.now()}`;
  const localBrepSerialized = normalizeFreeformSerialized(source);
  const geometry = new FreeformGeometry(
    new OGFreeformGeometry(id, localBrepSerialized)
  );

  if (options?.placement) {
    geometry.setPlacement(toObjectTransformation(options.placement));
  }

  return geometry;
}
