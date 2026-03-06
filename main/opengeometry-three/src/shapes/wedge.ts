import { OGWedge, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

export interface IWedgeOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: number;
}

export class Wedge extends THREE.Mesh {
  ogid: string;
  options: IWedgeOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    height: 1,
    depth: 1,
    color: 0x00ff00,
  };

  private wedge: OGWedge;
  #outlineMesh: THREE.Line | null = null;

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

  setConfig(options: IWedgeOptions) {
    this.validateOptions();

    const { width, height, depth, center, color } = options;
    this.wedge.set_config(center.clone(), width, height, depth);
    this.options.color = color;

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

    if (this.#outlineMesh) {
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

  getHlrOutlineGeometry(
    camera: THREE.PerspectiveCamera,
    target: THREE.Vector3,
    hideHiddenEdges = true
  ) {
    const kernel = this.wedge as unknown as {
      get_outline_geometry_hlr_serialized?: Function;
      get_outline_geometry_serialized: () => string;
    };

    if (typeof kernel.get_outline_geometry_hlr_serialized === "function") {
      const serialized = kernel.get_outline_geometry_hlr_serialized(
        new Vector3(camera.position.x, camera.position.y, camera.position.z),
        new Vector3(target.x, target.y, target.z),
        new Vector3(camera.up.x, camera.up.y, camera.up.z),
        camera.near,
        hideHiddenEdges
      );
      return JSON.parse(serialized) as number[];
    }

    const fallback = kernel.get_outline_geometry_serialized();
    return JSON.parse(fallback) as number[];
  }

  set outline(enable: boolean) {
    if (this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      this.#outlineMesh = null;
    }

    if (enable) {
      const outlineBuffer = this.wedge.get_outline_geometry_serialized();
      const outlineData = JSON.parse(outlineBuffer);

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

  get outlineMesh() {
    return this.#outlineMesh;
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}
