import { OGSimpleLine, Vector3D, CircleArc, OGRectangle } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { RectangeOptions } from "../base-types";

/**
 * Rectangle
 */
export class Rectangle extends THREE.Line {
  ogid: string;
  polyLineRectangle: OGRectangle;
  options: RectangeOptions;
  // nodeChild: RectanglePoly | null = null;
  nodeOperation: String = "none";
  #color: number = 0x000000;
  
  set width(width: number) {
    this.options.width = width;
    this.polyLineRectangle.update_width(width);
    this.generateGeometry();
  }

  set breadth(breadth: number) {
    this.options.breadth = breadth;
    this.polyLineRectangle.update_breadth(breadth);
    this.generateGeometry();
  }

  set color(color: number) {
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
      this.#color = color;
    }
  }

  constructor(options: RectangeOptions) {
    super();
    this.ogid = getUUID();
    this.options = options;
    this.polyLineRectangle = new OGRectangle(this.ogid);

    this.setConfig();
    this.generateGeometry();
  }

  setConfig() {
    const { breadth, width, center } = this.options;
    this.polyLineRectangle.set_config(
      center,
      width,
      breadth
    );
  }

  generateGeometry() {
    this.polyLineRectangle.generate_points();
    const bufRaw = this.polyLineRectangle.get_points();
    const bufFlush = JSON.parse(bufRaw);
    const line = new THREE.BufferGeometry().setFromPoints(bufFlush);
    const material = new THREE.LineBasicMaterial({ color: this.#color });
    this.geometry = line;
    this.material = material;
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}