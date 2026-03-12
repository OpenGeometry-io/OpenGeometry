import { OGWedge, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";

export interface IWedgeOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

export type WedgeConfigUpdate = Partial<IWedgeOptions>;

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

    this.setConfig(this.options);
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

  cleanGeometry() {
    this.geometry.dispose();
    if (Array.isArray(this.material)) {
      this.material.forEach((mat) => mat.dispose());
    } else {
      this.material.dispose();
    }
  }

  generateGeometry() {
    this.cleanGeometry();

    const geometryData = this.wedge.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    const material = new THREE.MeshStandardMaterial({
      color: this.options.color,
      transparent: true,
      opacity: 0.6,
    });

    geometry.computeVertexNormals();
    geometry.computeBoundingBox();

    this.geometry = geometry;
    this.material = material;

    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  getBrepData() {
    const brepData = this.wedge.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this wedge.");
    }
    return JSON.parse(brepData);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outlineBuffer = this.wedge.get_outline_geometry_serialized();
      const outlineData = JSON.parse(outlineBuffer) as number[];
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
}
