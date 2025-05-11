import { OGCylinder, Vector3D } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

interface ICylinderOptions {
  radius: number;
  height: number;
  segments: number;
  angle: number;
  center?: Vector3D;
}

export class Cylinder extends THREE.Mesh {
  ogid: string;
  options: ICylinderOptions;
  cylinder: OGCylinder;

  #outlineMesh: THREE.Line | null = null;
  #isOutline: boolean = false;

  constructor(options: ICylinderOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;

    this.cylinder = new OGCylinder(this.ogid);
    this.setConfig();
    this.generateGeometry();
  }

  validateOptions() {
    console.log(this.options);
    if (!this.options) {
      throw new Error("Options are not defined for Cylinder");
    }
  }

  setConfig() {
    this.validateOptions();

    const { radius, height, segments, angle, center } = this.options;
    this.options.center = center || new Vector3D(0, 0, 0);
    this.cylinder.set_config(
      this.options.center,
      radius,
      height,
      angle,
      segments
    );
  }

  generateGeometry() {
    this.cylinder.generate_geometry();
    const geometryData = this.cylinder.get_geometry();
    const bufferData = JSON.parse(geometryData);
    console.log(bufferData);

    const geometry = new THREE.BufferGeometry();
      // .setFromPoints(bufferData.vertices)
      // .setIndex(bufferData.indices);

    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );
    const material = new THREE.MeshStandardMaterial({
      color: 0x00ff00, 
      // side: THREE.DoubleSide, 
      transparent: true, 
      opacity: 0.5, 
      // wireframe: true
    });

    geometry.computeVertexNormals();

    this.geometry = geometry;
    this.material = material;
  }

  set outline(enable: boolean) {
    if (!this.outline) {
      const outline_buff = this.cylinder.outline_edges();
      const outline_buf = JSON.parse(outline_buff);
      console.log(outline_buf);

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
      this.#isOutline = true;
    }
  }
}