import { OGCuboid, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";

export interface ICuboidOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

export class Cuboid extends THREE.Mesh {
  ogid: string;
  options: ICuboidOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    height: 1,
    depth: 1,
    color: 0x00ff00,
    fatOutlines: false,
    outlineWidth: 1,
  };

  private cuboid: OGCuboid;
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

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ICuboidOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.cuboid = new OGCuboid(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Cylinder");
    }
  }

  setConfig(options: ICuboidOptions) {
    this.validateOptions();

    this.options = { ...this.options, ...options };
    this._fatOutlines = this.options.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(this.options.outlineWidth);
    this.options.fatOutlines = this._fatOutlines;
    this.options.outlineWidth = this._outlineWidth;

    const { width, height, depth, center, color } = this.options;
    this.cuboid.set_config(
      center.clone(),
      width,
      height,
      depth
    );

    this.options.color = color;

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
    // Since Kernel Generates Geometry if we do set_config, we don't need to generate Geometry separately. We can directly get the geometry data and create Three.js geometry.
    const geometryData = this.cuboid.get_geometry_serialized();
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

  getBrepData() {
    if (!this.cuboid) return null;
    const brepData = this.cuboid.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this cuboid.");
    }
    return JSON.parse(brepData);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outline_buff = this.cuboid.get_outline_geometry_serialized();
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
