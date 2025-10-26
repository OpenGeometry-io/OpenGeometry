import { OGCuboid, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

interface IOpeningOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: number;
}

export class Opening extends THREE.Mesh {
  ogid: string;
  options: IOpeningOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    height: 1,
    depth: 0.2,
    color: 0xdad7cd,
  }
  
  private opening: OGCuboid;
  #outlineMesh: THREE.Line | null = null;

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

  setConfig(options: IOpeningOptions) {
    this.validateOptions();

    const { width, height, depth, center } = options;
    this.opening.set_config(
      center?.clone(),
      width,
      height,
      depth
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
    if (this.#outlineMesh) {
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

  set outline(enable: boolean) {
    if (this.#outlineMesh) {
      this.remove(this.#outlineMesh);
      this.#outlineMesh.geometry.dispose();
      this.#outlineMesh = null;
    }

    if (enable) {
      const outline_buff = this.opening.get_outline_geometry_serialized();
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
