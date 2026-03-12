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

export interface ISweepOptions {
  ogid?: string;
  path: Vector3[];
  profile: Vector3[];
  color: number;
  capStart?: boolean;
  capEnd?: boolean;
  fatOutlines?: boolean;
  outlineWidth?: number;
}

export type SweepConfigUpdate = Partial<ISweepOptions>;

/* eslint-disable no-unused-vars */
interface ISweepKernelInstance {
  set_config_with_caps: (
    ..._args: [Vector3[], Vector3[], boolean, boolean]
  ) => void;
  get_geometry_serialized(): string;
  get_brep_serialized(): string;
  get_outline_geometry_serialized(): string;
}

type SweepKernelConstructor = new (..._args: [string]) => ISweepKernelInstance;
/* eslint-enable no-unused-vars */

export class Sweep extends THREE.Mesh {
  ogid: string;
  options: ISweepOptions = {
    path: [
      new Vector3(0, 0, 0),
      new Vector3(0, 1, 0),
    ],
    profile: [
      new Vector3(-0.25, 0, -0.25),
      new Vector3(0.25, 0, -0.25),
      new Vector3(0.25, 0, 0.25),
      new Vector3(-0.25, 0, 0.25),
    ],
    color: 0x00ff00,
    capStart: true,
    capEnd: true,
    fatOutlines: false,
    outlineWidth: 1,
  };

  private sweep: ISweepKernelInstance;
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

  constructor(options?: ISweepOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    const sweepExport = (OGKernel as Record<string, unknown>)["OGSweep"];
    if (typeof sweepExport !== "function") {
      throw new Error(
        "OGSweep is not available in the loaded wasm package. Rebuild opengeometry wasm bindings."
      );
    }
    const SweepKernel = sweepExport as SweepKernelConstructor;
    this.sweep = new SweepKernel(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Sweep");
    }

    if (this.options.path.length < 2) {
      throw new Error("Sweep path requires at least 2 points.");
    }

    if (this.options.profile.length < 3) {
      throw new Error("Sweep profile requires at least 3 points.");
    }
  }

  setConfig(options: SweepConfigUpdate) {
    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "path" in options ||
      "profile" in options ||
      "capStart" in options ||
      "capEnd" in options;
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
      this.validateOptions();

      const path = this.options.path.map((point) => point.clone());
      const profile = this.options.profile.map((point) => point.clone());
      const capStart = this.options.capStart ?? true;
      const capEnd = this.options.capEnd ?? true;

      this.sweep.set_config_with_caps(path, profile, capStart, capEnd);
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

    const geometryData = this.sweep.get_geometry_serialized();
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
    const brepData = this.sweep.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for Sweep.");
    }
    return JSON.parse(brepData);
  }

  set outline(enable: boolean) {
    this._outlineEnabled = enable;
    this.clearOutlineMesh();
    if (enable) {
      const outlineBuff = this.sweep.get_outline_geometry_serialized();
      const outlineData = JSON.parse(outlineBuff) as number[];
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
