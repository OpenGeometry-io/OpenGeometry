import { OGCylinder, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";

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
}

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
  };
  
  private cylinder: OGCylinder;
  #outlineMesh: ShapeOutlineMesh | null = null;
  private _outlineEnabled = false;
  private _fatOutlines = false;
  private _outlineWidth = 1;

  set radius(value: number) {
    this.options.radius = value;
    this.setConfig(this.options);
  }

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ICylinderOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.cylinder = new OGCylinder(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Cylinder");
    }
  }

  setConfig(options: ICylinderOptions) {
    this.validateOptions();

    this.options = { ...this.options, ...options };
    this._fatOutlines = this.options.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(this.options.outlineWidth);
    this.options.fatOutlines = this._fatOutlines;
    this.options.outlineWidth = this._outlineWidth;

    const { radius, height, segments, angle, center } = this.options;
    this.cylinder.set_config(
      center?.clone(),
      radius,
      height,
      angle,
      segments
    );

    this.generateGeometry();
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
    this.cleanGeometry();

    // Kernel Geometry
    // Since geometry is already generated in set_config, we don't need to call it again
    // this.cylinder.generate_geometry();

    const geometryData = this.cylinder.get_geometry_serialized();
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

    // outline
    if (this._outlineEnabled) {
      this.outline = true;
    }
  }

  getBrep() {
    if (!this.cylinder) return null;
    const brepData = this.cylinder.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this cylinder.");
    }
    return JSON.parse(brepData);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outline_buff = this.cylinder.get_outline_geometry_serialized();
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
