import { OGCylinder, Vector3 } from "../../../opengeometry/pkg/opengeometry";
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
  ShapeSubtractOperand,
  ShapeSubtractOptions,
  ShapeSubtractResult,
} from "./boolean-subtract";

/**
 * Construction options for a cylinder shape.
 */
export interface ICylinderOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  height: number;
  segments: number;
  angle: number;
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Placement updates accepted by `Cylinder`.
 */
export interface CylinderPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Partial config payload accepted by `Cylinder.setConfig(...)`.
 */
export type CylinderConfigUpdate = Partial<
  Omit<ICylinderOptions, "translation" | "rotation" | "scale">
>;

/**
 * Alias for `Cylinder` placement updates.
 */
export type CylinderPlacementUpdate = CylinderPlacementOptions;

/**
 * Cylinder wrapper backed by the kernel cylindrical primitive.
 */
export class Cylinder extends THREE.Mesh {
  ogid: string;
  options: ICylinderOptions = {
    center: new Vector3(0, 0, 0),
    radius: 1,
    height: 1,
    segments: 32,
    angle: 2 * Math.PI,
    color: 0x00ff00,
    fatOutlines: false,
    outlineWidth: 1,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };
  
  private cylinder: OGCylinder;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;

  set radius(value: number) {
    this.setConfig({ radius: value });
  }

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ICylinderOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.cylinder = new OGCylinder(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      center: this.options.center.clone(),
      radius: this.options.radius,
      height: this.options.height,
      segments: this.options.segments,
      angle: this.options.angle,
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
      throw new Error("Options are not defined for Cylinder");
    }
  }

  setConfig(options: CylinderConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "center" in options ||
      "radius" in options ||
      "height" in options ||
      "segments" in options ||
      "angle" in options;
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
      const { radius, height, segments, angle, center } = this.options;
      this.cylinder.set_config(
        center.clone(),
        radius,
        height,
        angle,
        segments
      );

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
    const anchor = this.cylinder.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setPlacement(placement: CylinderPlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.cylinder.set_transform(
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
    return createParametricEditCapabilities("cylinder", "radial");
  }

  canConvertToFreeform() {
    return true;
  }

  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.cylinder.get_local_brep_serialized(), {
      id,
      placement: this.getPlacement(),
    });
  }

  cleanGeometry() {
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach(mat => mat.dispose());
    } else {
      this.material.dispose();
    }
  }

  generateGeometry() {
    const bufferData = Array.from(this.cylinder.get_geometry_buffer());

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

    this.geometry.computeVertexNormals();
    this.geometry.computeBoundingBox();
    this.geometry.computeBoundingSphere();

    if (!this._outlineEnabled) {
      return;
    }

    const outlineData = Array.from(this.cylinder.get_outline_geometry_buffer());
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

  getBrep() {
    if (!this.cylinder) return null;
    const brepData = this.cylinder.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this cylinder.");
    }
    return JSON.parse(brepData);
  }

  /**
   * Subtracts another boolean operand, such as an Opening, from this cylinder.
   */
  subtract(
    operand: ShapeSubtractOperand,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(this, operand, options);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outline_buf = Array.from(this.cylinder.get_outline_geometry_buffer());
      this.#outlineMesh = createShapeOutlineMesh({
        positions: outline_buf,
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
