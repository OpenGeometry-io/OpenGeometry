import {
  OGFreeformEditor as KernelFreeformEditor,
  Vector3,
} from "../../../opengeometry/pkg/opengeometry";

import { FreeformGeometry } from "../freeform";
import type { ObjectTransformation } from "../freeform/types";
import type {
  EdgeInfo,
  EditOperationOptions,
  FaceInfo,
  FreeformEditCapabilities,
  FreeformEditResult,
  FreeformFeatureEditCapabilities,
  TopologyId,
  TopologyRenderData,
  TopologyVertexRenderData,
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

function normalizeTopologyRenderData(
  topology: TopologyRenderData
): TopologyRenderData {
  return {
    ...topology,
    vertices: topology.vertices.map((vertex) =>
      normalizeTopologyVertex(vertex)
    ),
  };
}

function normalizeTopologyVertex(
  vertex: TopologyVertexRenderData
): TopologyVertexRenderData {
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
    canEditPlacement: false,
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

export class FreeformEditor {
  private readonly geometry: FreeformGeometry;
  private readonly editor: KernelFreeformEditor;

  constructor(
    geometry: FreeformGeometry,
    editor: KernelFreeformEditor = new KernelFreeformEditor()
  ) {
    this.geometry = geometry;
    this.editor = editor;
  }

  getFreeformGeometry(): FreeformGeometry {
    return this.geometry;
  }

  getTopologyRenderData(): TopologyRenderData {
    return normalizeTopologyRenderData(
      parseJson<TopologyRenderData>(
        this.editor.getTopologyRenderData(this.geometry.getKernelGeometry())
      )
    );
  }

  getFaceInfo(faceId: TopologyId): FaceInfo {
    return normalizeFaceInfo(
      parseJson<FaceInfo>(
        this.editor.getFaceInfo(this.geometry.getKernelGeometry(), faceId)
      )
    );
  }

  getEdgeInfo(edgeId: TopologyId): EdgeInfo {
    return normalizeEdgeInfo(
      parseJson<EdgeInfo>(
        this.editor.getEdgeInfo(this.geometry.getKernelGeometry(), edgeId)
      )
    );
  }

  getVertexInfo(vertexId: TopologyId): VertexInfo {
    return normalizeVertexInfo(
      parseJson<VertexInfo>(
        this.editor.getVertexInfo(this.geometry.getKernelGeometry(), vertexId)
      )
    );
  }

  getEditCapabilities(): FreeformEditCapabilities {
    return normalizeFreeformCapabilities(
      parseJson<RawFreeformOperationCapabilities>(
        this.editor.getEditCapabilities(this.geometry.getKernelGeometry())
      )
    );
  }

  getFaceEditCapabilities(faceId: TopologyId): FreeformFeatureEditCapabilities {
    return normalizeFeatureCapabilities(
      parseJson<RawFreeformFeatureEditCapabilities>(
        this.editor.getFaceEditCapabilities(this.geometry.getKernelGeometry(), faceId)
      )
    );
  }

  getEdgeEditCapabilities(edgeId: TopologyId): FreeformFeatureEditCapabilities {
    return normalizeFeatureCapabilities(
      parseJson<RawFreeformFeatureEditCapabilities>(
        this.editor.getEdgeEditCapabilities(this.geometry.getKernelGeometry(), edgeId)
      )
    );
  }

  getVertexEditCapabilities(
    vertexId: TopologyId
  ): FreeformFeatureEditCapabilities {
    return normalizeFeatureCapabilities(
      parseJson<RawFreeformFeatureEditCapabilities>(
        this.editor.getVertexEditCapabilities(
          this.geometry.getKernelGeometry(),
          vertexId
        )
      )
    );
  }

  pushPullFace(
    faceId: TopologyId,
    distance: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.pushPullFace(
        this.geometry.getKernelGeometry(),
        faceId,
        distance,
        serializeOptions(options)
      )
    );
  }

  extrudeFace(
    faceId: TopologyId,
    distance: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.extrudeFace(
        this.geometry.getKernelGeometry(),
        faceId,
        distance,
        serializeOptions(options)
      )
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
      this.editor.cutFace(
        this.geometry.getKernelGeometry(),
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
      this.editor.moveFace(
        this.geometry.getKernelGeometry(),
        faceId,
        translation,
        serializeOptions(options)
      )
    );
  }

  moveEdge(
    edgeId: TopologyId,
    translation: Vector3,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.moveEdge(
        this.geometry.getKernelGeometry(),
        edgeId,
        translation,
        serializeOptions(options)
      )
    );
  }

  moveVertex(
    vertexId: TopologyId,
    translation: Vector3,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.moveVertex(
        this.geometry.getKernelGeometry(),
        vertexId,
        translation,
        serializeOptions(options)
      )
    );
  }

  insertVertexOnEdge(
    edgeId: TopologyId,
    t: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.insertVertexOnEdge(
        this.geometry.getKernelGeometry(),
        edgeId,
        t,
        serializeOptions(options)
      )
    );
  }

  splitEdge(
    edgeId: TopologyId,
    t: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.splitEdge(
        this.geometry.getKernelGeometry(),
        edgeId,
        t,
        serializeOptions(options)
      )
    );
  }

  loopCut(
    edgeId: TopologyId,
    t: number,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.loopCut(
        this.geometry.getKernelGeometry(),
        edgeId,
        t,
        serializeOptions(options)
      )
    );
  }

  removeVertex(
    vertexId: TopologyId,
    options?: EditOperationOptions
  ): FreeformEditResult {
    return parseEditResult(
      this.editor.removeVertex(
        this.geometry.getKernelGeometry(),
        vertexId,
        serializeOptions(options)
      )
    );
  }
}

export function createFreeformEditor(
  geometry: FreeformGeometry
): FreeformEditor {
  return new FreeformEditor(geometry);
}
