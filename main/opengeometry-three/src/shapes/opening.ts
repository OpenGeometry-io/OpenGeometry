import { OGCuboid, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
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
}

export type OpeningConfigUpdate = Partial<IOpeningOptions>;

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
  }
  
  private opening: OGCuboid;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;

  // Store local center offset to align outlines
  // TODO: Can this be moved to Engine? It can increase performance | Needs to be used in other shapes too
  private _geometryCenterOffset = new THREE.Vector3();

  set width(value: number) {
    this.options.width = value;
    this.setConfig(this.options);
  }

  set height(value: number) {
    this.options.height = value;
    this.setConfig(this.options);
  }

  set depth(value: number) {
    this.options.depth = value;
    this.setConfig(this.options);
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

    this.setConfig(this.options);
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

  cleanGeometry() {
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach(mat => mat.dispose());
    } else {
      this.material.dispose();
    }
  }

  generateGeometry() {
    // Three.js cleanup
    this.cleanGeometry();

    // Kernel Geometry
    this.opening.generate_geometry();
    const geometryData = this.opening.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    const material = new THREE.MeshStandardMaterial({
      color: this.options.color,
      transparent: true,
      opacity: 0,
      // Disable depth writing for transparent materials, so that we can see through openings and elements behind them
      depthWrite: false,
    });

    geometry.computeVertexNormals();
    geometry.computeBoundingBox();

    this.geometry = geometry;
    this.material = material;

    // outline
    if (this._outlineEnabled) {
      this.outline = true;
    }
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
      const outline_buff = this.opening.get_outline_geometry_serialized();
      const outline_buf = JSON.parse(outline_buff) as number[];
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
}
