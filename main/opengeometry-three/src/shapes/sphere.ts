import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import {
  createShapeOutlineMesh,
  disposeShapeOutlineMesh,
  sanitizeOutlineWidth,
  ShapeOutlineMesh,
} from "./outline-utils";

export interface ISphereOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  widthSegments: number;
  heightSegments: number;
  color: number;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

/* eslint-disable no-unused-vars */
interface ISphereKernelInstance {
  set_config: (
    ..._args: [Vector3, number, number, number]
  ) => void;
  get_geometry_serialized(): string;
  get_brep_serialized(): string;
  get_outline_geometry_serialized(): string;
}

type SphereKernelConstructor = new (..._args: [string]) => ISphereKernelInstance;
/* eslint-enable no-unused-vars */

export class Sphere extends THREE.Mesh {
  ogid: string;
  options: ISphereOptions = {
    center: new Vector3(0, 0, 0),
    radius: 1,
    widthSegments: 24,
    heightSegments: 16,
    color: 0x00ff00,
    fatOutlines: false,
    outlineWidth: 1,
  };

  private sphere: ISphereKernelInstance;
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
    if (this.material instanceof THREE.MeshStandardMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ISphereOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();

    const sphereKey = ["OG", "Sphere"].join("");
    const sphereExport = (OGKernel as Record<string, unknown>)[sphereKey];
    if (typeof sphereExport !== "function") {
      throw new Error(
        "OGSphere is not available in the loaded wasm package. Rebuild opengeometry wasm bindings."
      );
    }
    const SphereKernel = sphereExport as SphereKernelConstructor;
    this.sphere = new SphereKernel(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Sphere");
    }
  }

  setConfig(options: ISphereOptions) {
    this.validateOptions();

    this.options = { ...this.options, ...options };
    this._fatOutlines = this.options.fatOutlines ?? false;
    this._outlineWidth = sanitizeOutlineWidth(this.options.outlineWidth);
    this.options.fatOutlines = this._fatOutlines;
    this.options.outlineWidth = this._outlineWidth;

    this.sphere.set_config(
      this.options.center.clone(),
      this.options.radius,
      this.options.widthSegments,
      this.options.heightSegments
    );

    this.generateGeometry();
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

    const geometryData = this.sphere.get_geometry_serialized();
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

  getBrep() {
    const brepData = this.sphere.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this sphere.");
    }
    return JSON.parse(brepData);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outlineBuffer = this.sphere.get_outline_geometry_serialized();
      const lineBuffer = JSON.parse(outlineBuffer) as number[];
      this.#outlineMesh = createShapeOutlineMesh({
        positions: lineBuffer,
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
