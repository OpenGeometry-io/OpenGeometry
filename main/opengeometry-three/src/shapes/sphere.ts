import * as OGKernel from "../../../opengeometry/pkg/opengeometry";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

export interface ISphereOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  widthSegments: number;
  heightSegments: number;
  color: number;
}

/* eslint-disable no-unused-vars */
interface ISphereKernelInstance {
  set_config: (
    ..._args: [Vector3, number, number, number]
  ) => void;
  get_geometry_serialized(): string;
  get_brep_serialized(): string;
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
  };

  private sphere: ISphereKernelInstance;
  #outlineMesh: THREE.LineSegments | null = null;

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

    if (this.#outlineMesh) {
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
      const brep = this.getBrep();
      const lineBuffer: number[] = [];

      for (const edge of brep.edges ?? []) {
        const start = brep.vertices?.[edge.v1]?.position;
        const end = brep.vertices?.[edge.v2]?.position;
        if (!start || !end) {
          continue;
        }

        lineBuffer.push(
          start.x, start.y, start.z,
          end.x, end.y, end.z
        );
      }

      const outlineGeometry = new THREE.BufferGeometry();
      outlineGeometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(lineBuffer, 3)
      );

      const outlineMaterial = new THREE.LineBasicMaterial({ color: 0x000000 });
      this.#outlineMesh = new THREE.LineSegments(outlineGeometry, outlineMaterial);
      this.add(this.#outlineMesh);
    }
  }

  get outlineMesh() {
    return this.#outlineMesh;
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}
