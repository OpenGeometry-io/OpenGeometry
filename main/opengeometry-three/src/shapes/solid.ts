import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";

import { clonePlacement, toObjectTransformation } from "../editor";
import { createFreeformGeometry, type FreeformGeometry } from "../freeform";
import type { FreeformSource, ObjectTransformation } from "../freeform/types";
import { extrudeBrepFace } from "../operations/extrude";
import { getUUID } from "../utils/randomizer";
import type {
  ShapeSubtractOperand,
  ShapeSubtractOptions,
  ShapeSubtractResult,
} from "./boolean-subtract";
import { subtractShapeOperand } from "./boolean-subtract";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  getShapeOutlineColor,
  sanitizeOutlineWidth,
  setShapeOutlineColor,
  ShapeOutlineMesh,
} from "./outline-utils";

/**
 * Construction options for a generic BRep-backed solid.
 */
export interface ISolidOptions {
  ogid?: string;
  brep: FreeformSource;
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
  anchor?: Vector3;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Placement updates accepted by `Solid`.
 */
export interface SolidPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Additional options accepted when creating a `Solid` through face extrusion.
 */
export interface ExtrudeSolidOptions
  extends Omit<ISolidOptions, "brep" | "ogid" | "anchor" | "translation" | "rotation" | "scale"> {
  ogid?: string;
  anchor?: Vector3;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Generic renderable BRep-backed solid used for extrusion outputs and other
 * kernel results that are not dedicated parametric shapes.
 */
export class Solid extends THREE.Mesh {
  ogid: string;
  options: ISolidOptions = {
    brep: "{}",
    color: 0x00ff00,
    fatOutlines: false,
    outlineWidth: 1,
    anchor: new Vector3(0, 0, 0),
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };

  private solid: FreeformGeometry;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;
  private _outlineColor = 0x000000;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  get color() {
    return this.options.color;
  }

  constructor(options: ISolidOptions) {
    super();

    this.ogid = options.ogid ?? getUUID();
    this.options = {
      ...this.options,
      ...options,
      ogid: this.ogid,
      anchor: cloneVector(options.anchor, [0, 0, 0]),
      translation: cloneVector(options.translation, [0, 0, 0]),
      rotation: cloneVector(options.rotation, [0, 0, 0]),
      scale: cloneVector(options.scale, [1, 1, 1]),
    };
    this._fatOutlines = this.options.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(this.options.outlineWidth);
    this.options.fatOutlines = this._fatOutlines;
    this.options.outlineWidth = this._outlineWidth;

    this.solid = createFreeformGeometry(this.options.brep, {
      id: this.ogid,
      placement: this.getPlacementTransform(),
    });
    this.generateGeometry();
  }

  /**
   * Extrudes a face-like BRep source and wraps the result as a renderable solid.
   */
  static extrude(
    source: FreeformSource,
    height: number,
    options: ExtrudeSolidOptions
  ) {
    return new Solid({
      ...options,
      brep: extrudeBrepFace(source, height),
    });
  }

  /**
   * Returns the current placement values as cloned vectors.
   */
  getPlacement() {
    return clonePlacement(this.getPlacementTransform());
  }

  /**
   * Applies translation, rotation, and scale updates to the wrapped BRep.
   */
  setPlacement(placement: SolidPlacementOptions) {
    this.options.translation = cloneVector(
      placement.translation ?? this.options.translation,
      [0, 0, 0]
    );
    this.options.rotation = cloneVector(
      placement.rotation ?? this.options.rotation,
      [0, 0, 0]
    );
    this.options.scale = cloneVector(
      placement.scale ?? this.options.scale,
      [1, 1, 1]
    );

    this.solid.setPlacement(this.getPlacementTransform());
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

  getAnchor() {
    return this.getPlacementTransform().anchor.clone();
  }

  setAnchor(anchor: Vector3) {
    this.options.anchor = anchor.clone();
    this.solid.setPlacement(this.getPlacementTransform());
    this.generateGeometry();
  }

  /**
   * Solid is always convertible back to the freeform editing wrapper.
   */
  canConvertToFreeform() {
    return true;
  }

  /**
   * Returns a freeform wrapper for direct editing of this solid's BRep.
   */
  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.getLocalBrepSerialized(), {
      id,
      placement: this.getPlacementTransform(),
    });
  }

  /**
   * Returns the world-space serialized BRep payload for this solid.
   */
  getBrepSerialized() {
    return this.solid.getBrepSerialized();
  }

  /**
   * Returns the local-space serialized BRep payload for this solid.
   */
  getLocalBrepSerialized() {
    return this.solid.getLocalBrepSerialized();
  }

  /**
   * Returns the parsed world-space BRep object for this solid.
   */
  getBrepData() {
    const brepSerialized = this.getBrepSerialized();
    if (!brepSerialized) {
      throw new Error("Brep data is not available for this solid.");
    }
    return JSON.parse(brepSerialized);
  }

  /**
   * Subtracts another boolean operand from this solid.
   */
  subtract(
    operand: ShapeSubtractOperand,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(this, operand, options);
  }

  /**
   * Rebuilds the visible mesh and outline from the current freeform geometry.
   */
  generateGeometry() {
    const bufferData = this.solid.getGeometryBuffer();
    this.writePositionsToGeometry(this.geometry, bufferData);

    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(this.options.color);
      this.material.transparent = true;
      this.material.opacity = 0.6;
    } else {
      if (Array.isArray(this.material)) {
        this.material.forEach((material) => material.dispose());
      } else {
        this.material.dispose();
      }

      this.material = new THREE.MeshStandardMaterial({
        color: this.options.color,
        transparent: true,
        opacity: 0.6,
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

    const outlineData = this.solid.getOutlineGeometryBuffer();
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
      this.#outlineMesh = createShapeOutlineMesh({
        positions: this.solid.getOutlineGeometryBuffer(),
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

  clearOutlineMesh() {
    if (!this.#outlineMesh) {
      return;
    }
    this.remove(this.#outlineMesh);
    disposeShapeOutlineMesh(this.#outlineMesh);
    this.#outlineMesh = null;
  }

  private getPlacementTransform(): ObjectTransformation {
    return toObjectTransformation({
      anchor: cloneVector(this.options.anchor, [0, 0, 0]),
      translation: cloneVector(this.options.translation, [0, 0, 0]),
      rotation: cloneVector(this.options.rotation, [0, 0, 0]),
      scale: cloneVector(this.options.scale, [1, 1, 1]),
    });
  }

  private writePositionsToGeometry(
    geometry: THREE.BufferGeometry,
    positions: number[]
  ) {
    const attribute = geometry.getAttribute("position");
    if (
      !(attribute instanceof THREE.BufferAttribute) ||
      attribute.itemSize !== 3 ||
      attribute.count !== positions.length / 3
    ) {
      geometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(positions, 3)
      );
      return;
    }

    (attribute.array as Float32Array).set(positions);
    attribute.needsUpdate = true;
  }
}

function cloneVector(
  vector: Vector3 | undefined,
  fallback: [number, number, number]
): Vector3 {
  return vector?.clone() ?? new Vector3(...fallback);
}
