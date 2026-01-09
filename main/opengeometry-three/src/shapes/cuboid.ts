import { OGCuboid, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

export interface ICuboidOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: number;
}

export class Cuboid extends THREE.Mesh {
  ogid: string;
  options: ICuboidOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    height: 1,
    depth: 1,
    color: 0x00ff00,
  };

  private cuboid: OGCuboid;
  #outlineMesh: THREE.Line | null = null;

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

    const { width, height, depth, center, color } = options;
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
    // Since geometry is already generated in set_config, we don't need to call it again
    // this.cuboid.generate_geometry();
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
    if (this.#outlineMesh) {
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
    if (this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      this.#outlineMesh = null;
    }

    if (enable) {
      const outline_buff = this.cuboid.get_outline_geometry_serialized();
      const outline_buf = JSON.parse(outline_buff);

      const outlineGeometry = new THREE.BufferGeometry();
      outlineGeometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(outline_buf, 3)
      );

      const outlineMaterial = new THREE.LineBasicMaterial({ color: 0x000000 });
      this.#outlineMesh = new THREE.LineSegments(
        outlineGeometry,
        outlineMaterial
      );

      this.add(this.#outlineMesh);
    }

    if (!enable && this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      this.#outlineMesh = null;
    }
  }

  get outlineMesh() {
    return this.#outlineMesh;
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}
