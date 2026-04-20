import { OGWedge, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";
import {
  clonePlacement,
  createParametricEditCapabilities,
} from "../editor";
import { createFreeformGeometry } from "../freeform";
import { subtractShapeOperand } from "./boolean-subtract";
import type {
  ShapeSubtractOperands,
  ShapeSubtractOptions,
  ShapeSubtractResult,
} from "./boolean-subtract";

/**
 * Construction options for a wedge/prism shape.
 */
export interface IWedgeOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Placement updates accepted by `Wedge`.
 */
export interface WedgePlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Partial config payload accepted by `Wedge.setConfig(...)`.
 */
export type WedgeConfigUpdate = Partial<
  Omit<IWedgeOptions, "translation" | "rotation" | "scale">
>;

/**
 * Alias for `Wedge` placement updates.
 */
export type WedgePlacementUpdate = WedgePlacementOptions;

/**
 * Wedge wrapper backed by the kernel wedge primitive.
 */
export class Wedge extends THREE.Mesh {
  ogid: string;
  options: IWedgeOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    height: 1,
    depth: 1,
    color: 0x00ff00,
    fatOutlines: false,
    outlineWidth: 1,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };

  private wedge: OGWedge;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: IWedgeOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.wedge = new OGWedge(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      center: this.options.center.clone(),
      width: this.options.width,
      height: this.options.height,
      depth: this.options.depth,
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
      throw new Error("Options are not defined for Wedge");
    }
  }

  setConfig(options: WedgeConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "center" in options ||
      "width" in options ||
      "height" in options ||
      "depth" in options;
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
      const { width, height, depth, center } = this.options;
      this.wedge.set_config(center.clone(), width, height, depth);
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
    const anchor = this.wedge.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setPlacement(placement: WedgePlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.wedge.set_transform(
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

  getEditCapabilities() {
    return createParametricEditCapabilities("wedge", "wedge");
  }

  canConvertToFreeform() {
    return true;
  }

  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.wedge.get_local_brep_serialized(), {
      id,
      placement: this.getPlacement(),
    });
  }

  cleanGeometry() {
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach((mat) => mat.dispose());
    } else {
      this.material.dispose();
    }
  }

  generateGeometry() {
    const bufferData = Array.from(this.wedge.get_geometry_buffer());

    this.writePositionsToGeometry(this.geometry, bufferData);

    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(this.options.color);
      // this.material.transparent = true;
      // this.material.opacity = 0.6;
    } else {
      if (Array.isArray(this.material)) {
        this.material.forEach((material) => material.dispose());
      } else {
        this.material.dispose();
      }

      this.material = new THREE.MeshStandardMaterial({
        color: this.options.color,
        // transparent: true,
        // opacity: 0.6,
      });
    }

    this.geometry.computeVertexNormals();
    this.geometry.computeBoundingBox();
    this.geometry.computeBoundingSphere();

    if (!this._outlineEnabled) {
      return;
    }

    const outlineData = Array.from(this.wedge.get_outline_geometry_buffer());
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

  getBrepData() {
    const brepData = this.wedge.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this wedge.");
    }
    return JSON.parse(brepData);
  }

  /**
   * Subtracts one or more boolean operands, such as Opening volumes, from this wedge.
   */
  subtract(
    operands: ShapeSubtractOperands,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(this, operands, options);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outlineData = Array.from(this.wedge.get_outline_geometry_buffer());
      this.#outlineMesh = createShapeOutlineMesh({
        positions: outlineData,
        color: 0x000000,
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

  get outlineMesh() {
    return this.#outlineMesh;
  }

  private clearOutlineMesh() {
    if (!this.#outlineMesh) {
      return;
    }
    this.remove(this.#outlineMesh);
    disposeShapeOutlineMesh(this.#outlineMesh);
    this.#outlineMesh = null;
  }

  discardGeometry() {
    this.geometry.dispose();
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
}
