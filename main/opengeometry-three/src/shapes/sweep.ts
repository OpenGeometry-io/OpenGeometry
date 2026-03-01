import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

export interface ISweepOptions {
  ogid?: string;
  path: Vector3[];
  profile: Vector3[];
  color: number;
  capStart?: boolean;
  capEnd?: boolean;
}

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
  };

  private sweep: ISweepKernelInstance;
  #outlineMesh: THREE.LineSegments | null = null;

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

  setConfig(options: ISweepOptions) {
    this.options = { ...this.options, ...options };
    this.validateOptions();

    const path = this.options.path.map((point) => point.clone());
    const profile = this.options.profile.map((point) => point.clone());
    const capStart = this.options.capStart ?? true;
    const capEnd = this.options.capEnd ?? true;

    this.sweep.set_config_with_caps(path, profile, capStart, capEnd);
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

    if (this.#outlineMesh) {
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
    if (this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      if (Array.isArray(this.#outlineMesh.material)) {
        this.#outlineMesh.material.forEach((material) => material.dispose());
      } else {
        this.#outlineMesh.material.dispose();
      }
      this.#outlineMesh = null;
    }

    if (enable) {
      const outlineBuff = this.sweep.get_outline_geometry_serialized();
      const outlineData = JSON.parse(outlineBuff);

      const outlineGeometry = new THREE.BufferGeometry();
      outlineGeometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(outlineData, 3)
      );

      const outlineMaterial = new THREE.LineBasicMaterial({ color: 0x000000 });
      this.#outlineMesh = new THREE.LineSegments(outlineGeometry, outlineMaterial);

      this.add(this.#outlineMesh);
    }
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}
