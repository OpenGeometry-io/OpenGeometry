import { OGCuboid, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { LineSegmentsGeometry } from "three/examples/jsm/lines/LineSegmentsGeometry.js";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";
import { subtractShapeOperand } from "./boolean-subtract";
import type {
  ShapeSubtractOperand,
  ShapeSubtractOptions,
  ShapeSubtractResult,
} from "./boolean-subtract";

export interface IOpeningOptions {
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

export interface OpeningPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export type OpeningConfigUpdate = Partial<
  Omit<IOpeningOptions, "translation" | "rotation" | "scale">
>;
export type OpeningPlacementUpdate = OpeningPlacementOptions;

export class Opening extends THREE.Mesh {
  ogid: string;
  options: IOpeningOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    height: 1,
    depth: 0.2,
    color: 0xdad7cd,
    fatOutlines: false,
    outlineWidth: 1,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  }
  
  private opening: OGCuboid;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;

  set width(value: number) {
    this.setConfig({ width: value });
  }

  set height(value: number) {
    this.setConfig({ height: value });
  }

  set depth(value: number) {
    this.setConfig({ depth: value });
  }

  get dimensions() {
    return {
      width: this.options.width,
      height: this.options.height,
      depth: this.options.depth,
    };
  }

  constructor(options?: IOpeningOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.opening = new OGCuboid(this.ogid);

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
      throw new Error("Options are not defined for Opening");
    }
  }

  setConfig(options: OpeningConfigUpdate) {
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
      this.opening.set_config(
        center.clone(),
        width,
        height,
        depth
      );

      this.generateGeometry();
      return;
    }

    if (colorChanged && this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(this.options.color);
    }

    if (outlineStyleChanged && this._outlineEnabled) {
      this.outline = true;
    }
  }

  getAnchor() {
    const anchor = this.opening.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setPlacement(placement: OpeningPlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.opening.set_transform(
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

  cleanGeometry() {
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach(mat => mat.dispose());
    } else {
      this.material.dispose();
    }
  }

  generateGeometry() {
    const bufferData = Array.from(this.opening.get_geometry_buffer());

    this.writePositionsToGeometry(this.geometry, bufferData);

    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(this.options.color);
      this.material.transparent = true;
      this.material.opacity = 0;
      this.material.depthWrite = false;
    } else {
      if (Array.isArray(this.material)) {
        this.material.forEach((material) => material.dispose());
      } else {
        this.material.dispose();
      }

      this.material = new THREE.MeshStandardMaterial({
        color: this.options.color,
        transparent: true,
        opacity: 0,
        depthWrite: false,
      });
    }

    this.geometry.computeVertexNormals();
    this.geometry.computeBoundingBox();
    this.geometry.computeBoundingSphere();

    if (!this._outlineEnabled) {
      return;
    }

    const outlineData = Array.from(this.opening.get_outline_geometry_buffer());
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
    if (!this.opening) return null;
    const brepData = this.opening.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this opening.");
    }
    return JSON.parse(brepData);
  }

  /**
   * Subtracts another boolean operand from this opening volume.
   */
  subtract(
    operand: ShapeSubtractOperand,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(this, operand, options);
  }

  /**
   * Uses this opening volume to cut a host shape.
   */
  subtractFrom(
    host: ShapeSubtractOperand,
    options?: ShapeSubtractOptions
  ): ShapeSubtractResult {
    return subtractShapeOperand(host, this, options);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outline_buf = Array.from(this.opening.get_outline_geometry_buffer());
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
