import * as THREE from "three";
import { OGPolygon, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  getShapeOutlineColor,
  sanitizeOutlineWidth,
  setShapeOutlineColor,
  ShapeOutlineMesh,
} from "./outline-utils";
import {
  clonePlacement,
  createParametricEditCapabilities,
} from "../editor";
import { createFreeformGeometry } from "../freeform";
import { Solid } from "./solid";
import { subtractShapeOperand } from "./boolean-subtract";
import type {
  ShapeSubtractOperands,
  ShapeSubtractOptions,
  ShapeSubtractResult,
} from "./boolean-subtract";
import { extrudeBrepFace } from "../operations/extrude";

/**
 * Construction options for a planar polygon.
 */
export interface IPolygonOptions {
  ogid?: string;
  vertices: Vector3[];
  holes?: Vector3[][];
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export interface PolygonPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Partial polygon config updates that regenerate the profile when geometry changes.
 */
export type PolygonConfigUpdate = Partial<
  Omit<IPolygonOptions, "translation" | "rotation" | "scale">
>;

/**
 * Placement updates accepted by `Polygon`.
 */
export type PolygonPlacementUpdate = PolygonPlacementOptions;

/**
 * Planar polygon mesh with support for holes, booleans, and extrusion to `Solid`.
 */
export class Polygon extends THREE.Mesh {
  ogid: string;
  options: IPolygonOptions = {
    vertices: [],
    holes: [],
    color: 0x00ff00,
    fatOutlines: false,
    outlineWidth: 1,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };
  polygon: OGPolygon;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;
  private _outlineColor = 0x000000;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.MeshBasicMaterial) {
      this.material.color.set(color);
    }
  }

  get color() {
    return this.options.color;
  }

  constructor(options?: IPolygonOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.polygon = new OGPolygon(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      vertices: this.options.vertices.map((vertex) => vertex.clone()),
      holes: (this.options.holes ?? []).map((hole) =>
        hole.map((vertex) => vertex.clone())
      ),
      color: this.options.color,
      fatOutlines: this.options.fatOutlines,
      outlineWidth: this.options.outlineWidth,
    });
    this.setPlacement({
      translation: this.options.translation?.clone(),
      rotation: this.options.rotation?.clone(),
      scale: this.options.scale?.clone(),
    });
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Polygon");
    }
  }

  setConfig(options: PolygonConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "vertices" in options ||
      "holes" in options;
    const colorChanged = "color" in options;
    const outlineStyleChanged =
      "fatOutlines" in options ||
      "outlineWidth" in options;

    this.options = nextOptions;
    this._fatOutlines = this.options.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(this.options.outlineWidth);
    this.options.fatOutlines = this._fatOutlines;
    this.options.outlineWidth = this._outlineWidth;

    if (geometryChanged) {
      this.polygon.set_config(this.options.vertices.map((vertex) => vertex.clone()));
      (this.options.holes ?? []).forEach((hole) => {
        this.polygon.add_holes(hole.map((vertex) => vertex.clone()));
      });
      this.generateGeometry();
      return;
    }

    if (colorChanged) {
      this.color = this.options.color;
    }

    if (outlineStyleChanged && this._outlineEnabled) {
      this.outline = true;
    }
  }

  getConfig() {
    return this.options;
  }

  getAnchor() {
    const anchor = this.polygon.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setAnchor(anchor: Vector3) {
    this.polygon.set_anchor(anchor.clone());
    this.generateGeometry();
  }

  resetAnchor() {
    this.polygon.reset_anchor();
    this.generateGeometry();
  }

  setPlacement(placement: PolygonPlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.polygon.set_transform(
      this.options.translation?.clone() ?? new Vector3(0, 0, 0),
      this.options.rotation?.clone() ?? new Vector3(0, 0, 0),
      this.options.scale?.clone() ?? new Vector3(1, 1, 1)
    );
    this.generateGeometry();
  }

  setTransform(translation: Vector3, rotation: Vector3, scale: Vector3) {
    this.setPlacement({
      translation,
      rotation,
      scale,
    });
  }

  setTranslation(translation: Vector3) {
    this.setPlacement({ translation });
  }

  setRotation(rotation: Vector3) {
    this.setPlacement({ rotation });
  }

  setScale(scale: Vector3) {
    this.setPlacement({ scale });
  }

  getPlacement() {
    return clonePlacement({
      translation: this.options.translation,
      rotation: this.options.rotation,
      scale: this.options.scale,
    });
  }

  /**
   * Returns the editing capability payload advertised by parametric wrappers.
   */
  getEditCapabilities() {
    return createParametricEditCapabilities("polygon", "profile");
  }

  /**
   * Polygon always supports conversion into the freeform editing wrapper.
   */
  canConvertToFreeform() {
    return true;
  }

  /**
   * Returns a freeform wrapper for direct BRep editing of this polygon.
   */
  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.polygon.get_local_brep_serialized(), {
      id,
      placement: this.getPlacement(),
    });
  }

  cleanGeometry() {
    this.disposeResources();
  }

  generateGeometry() {
    const bufferData = Array.from(this.polygon.get_geometry_buffer());

    this.writePositionsToGeometry(this.geometry, bufferData);

    if (this.material instanceof THREE.MeshBasicMaterial) {
      this.material.color.set(this.options.color);
      this.material.side = THREE.DoubleSide;
    } else {
      if (Array.isArray(this.material)) {
        this.material.forEach((material) => material.dispose());
      } else {
        this.material.dispose();
      }

      this.material = new THREE.MeshBasicMaterial({
        color: this.options.color,
        side: THREE.DoubleSide,
      });
    }

    if (bufferData.length > 0) {
      this.geometry.computeVertexNormals();
      this.geometry.computeBoundingBox();
      this.geometry.computeBoundingSphere();
    } else {
      this.geometry.deleteAttribute("normal");
      this.geometry.boundingBox = null;
      this.geometry.boundingSphere = null;
    }

    if (!this._outlineEnabled) {
      return;
    }

    const outlineData = Array.from(this.polygon.get_outline_geometry_buffer());
    if (!this.#outlineMesh) {
      this.outline = true;
      return;
    }

    if (this.#outlineMesh instanceof THREE.LineSegments) {
      this.writePositionsToGeometry(this.#outlineMesh.geometry, outlineData);
      this.#outlineMesh.geometry.computeBoundingBox();
      this.#outlineMesh.geometry.computeBoundingSphere();
      return;
    }

    const fatGeometry = this.#outlineMesh.geometry as LineSegmentsGeometry;
    fatGeometry.setPositions(outlineData);

    const fatOutline = this.#outlineMesh as ShapeOutlineMesh & {
      computeLineDistances?: () => void;
    };
    fatOutline.computeLineDistances?.();
  }

  addVertices(vertices: Vector3[]) {
    if (!this.polygon) return;
    this.setConfig({
      vertices: vertices.map((vertex) => vertex.clone()),
    });
  }

  addHole(holeVertices: Vector3[]) {
    if (!this.polygon) return;
    this.options.holes = [
      ...(this.options.holes ?? []),
      holeVertices.map((vertex) => vertex.clone()),
    ];
    this.setConfig({
      holes: (this.options.holes ?? []).map((hole) =>
        hole.map((vertex) => vertex.clone())
      ),
    });
  }

  /**
   * Builds a kernel-backed `Solid` by extruding this polygon face by `height`.
   *
   * This is the recommended public path when a browser CAD or AEC workflow
   * starts from a planar profile and needs a boolean-ready solid.
   */
  extrude(height: number) {
    const config = this.getConfig();
    const placement = this.getPlacement();
    const solid = new Solid({
      brep: extrudeBrepFace(this.polygon.get_local_brep_serialized(), height),
      color: config.color,
      fatOutlines: config.fatOutlines,
      outlineWidth: config.outlineWidth,
      anchor: this.getAnchor(),
      translation: placement.translation.clone(),
      rotation: placement.rotation.clone(),
      scale: placement.scale.clone(),
    });

    solid.outline = this.outline;
    solid.outlineColor = this.outlineColor;
    return solid;
  }

  /**
   * Returns the serialized BRep payload for this polygon.
   */
  getBrepData() {
    if (!this.polygon) return null;
    const brepData = this.polygon.get_brep_serialized();
    return brepData;
  }

  /**
   * Subtracts one or more boolean operands from this polygon.
   */
  subtract(
    operands: ShapeSubtractOperands,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(this, operands, options);
  }

  set outlineColor(color: number) {
    this._outlineColor = color;
    setShapeOutlineColor(this.#outlineMesh, color);
  }

  get outlineColor() {
    return getShapeOutlineColor(this.#outlineMesh, this._outlineColor);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outline_buf = Array.from(this.polygon.get_outline_geometry_buffer());
      this.#outlineMesh = createShapeOutlineMesh({
        positions: outline_buf,
        color: this._outlineColor,
        fatOutlines: this._fatOutlines,
        outlineWidth: this._outlineWidth,
      });
      this.add(this.#outlineMesh);
    }
  }

  get outline() {
    return this._outlineEnabled;
  }

  set fatOutlines(value: boolean) {
    this._fatOutlines = value;
    this.options.fatOutlines = value;
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  get fatOutlines() {
    return this._fatOutlines;
  }

  set outlineWidth(value: number) {
    this._outlineWidth = sanitizeOutlineWidth(value);
    this.options.outlineWidth = this._outlineWidth;
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  get outlineWidth() {
    return this._outlineWidth;
  }

  disposeGeometryMaterial() {
    this.disposeResources();
  }

  private clearOutlineMesh() {
    if (!this.#outlineMesh) {
      return;
    }
    this.remove(this.#outlineMesh);
    disposeShapeOutlineMesh(this.#outlineMesh);
    this.#outlineMesh = null;
  }

  dispose() {
    this.disposeResources();
  }

  private writePositionsToGeometry(
    geometry: THREE.BufferGeometry,
    positions: number[]
  ) {
    const existing = geometry.getAttribute("position");
    if (
      !(existing instanceof THREE.BufferAttribute) ||
      existing.itemSize !== 3 ||
      existing.count !== positions.length / 3
    ) {
      geometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(positions, 3)
      );
      return;
    }

    const array = existing.array as Float32Array;
    array.set(positions);
    existing.needsUpdate = true;
  }

  private disposeResources() {
    if (this.geometry) {
      this.geometry.dispose();
    }
    if (Array.isArray(this.material)) {
      this.material.forEach((material) => material.dispose());
    } else if (this.material instanceof THREE.Material) {
      this.material.dispose();
    }
    this.clearOutlineMesh();
  }
}
