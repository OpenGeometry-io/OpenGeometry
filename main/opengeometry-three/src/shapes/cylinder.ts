import { OGCylinder, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

interface ICylinderOptions {
  radius: number;
  height: number;
  segments: number;
  angle: number;
  center?: Vector3;
}

export class Cylinder extends THREE.Mesh {
  ogid: string;
  options: ICylinderOptions;
  cylinder: OGCylinder;

  #outlineMesh: THREE.Line | null = null;

  constructor(options: ICylinderOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;

    this.cylinder = new OGCylinder(this.ogid);
    
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

    const { radius, height, segments, angle, center } = this.options;
    this.cylinder.set_config(
      center?.clone() || new Vector3(0, 0, 0),
      radius,
      height,
      angle,
      segments
    );
  }

  generateGeometry() {
    this.cylinder.generate_geometry();
    const geometryData = this.cylinder.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    
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
      const outline_buff = this.cylinder.get_outline_geometry_serialized();
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

  getBrep() {
    const brepData = this.cylinder.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for this cylinder.");
    }
    return JSON.parse(brepData);
  }
}
