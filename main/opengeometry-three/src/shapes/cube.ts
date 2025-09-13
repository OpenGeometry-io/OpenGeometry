import { OGCube, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

interface ICubeOptions {
  width: number;
  height: number;
  depth: number;
  center?: Vector3;
}

export class Cube extends THREE.Mesh {
  ogid: string;
  options: ICubeOptions;
  cube: OGCube;

  #outlineMesh: THREE.Line | null = null;

  // Store local center offset to align outlines
  // TODO: Can this be moved to Engine? It can increase performance | Needs to be used in other shapes too
  private _geometryCenterOffset = new THREE.Vector3();

  constructor(options: ICubeOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;

    this.cube = new OGCube(this.ogid);
    this.setConfig();
    this.generateGeometry();
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Cylinder");
    }
  }

  setConfig() {
    this.validateOptions();

    const { width, height, depth, center } = this.options;
    this.cube.set_config(
      center?.clone() || new Vector3(0, 0, 0),
      width,
      height,
      depth
    );
  }

  generateGeometry() {
    this.cube.generate_geometry();
    const geometryData = this.cube.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    console.log(bufferData);
    
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    const material = new THREE.MeshStandardMaterial({
      color: 0x00ff00,
      transparent: true,
      opacity: 0.6,
    });

    geometry.computeVertexNormals();
    geometry.computeBoundingBox();

    this.geometry = geometry;
    this.material = material;
  }

  set outline(enable: boolean) {
    if (enable && !this.#outlineMesh) {
      const outline_buff = this.cube.get_outline_geometry_serialized();
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
}
