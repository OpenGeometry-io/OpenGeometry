import { OGRectangle, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { RectangeOptions } from "../base-types";
import { BasePrimitive } from "./base-primitive";

/**
 * Rectangle
 */
export class Rectangle extends BasePrimitive {

  private polyLineRectangle: OGRectangle;

  options: RectangeOptions = {
    width: 1,
    breadth: 1,
    center: new Vector3(0, 0, 0),
    color: 0x000000,
  };

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

  set center(center: Vector3) {
    this.options.center = center;
    this.polyLineRectangle.update_center(center);
    this.generateGeometry();
  }

  set color(color: number) {
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
      this.options.color = color;
    }
  }

  constructor(options?: RectangeOptions) {
    super();

    const ogid = options?.ogid ?? getUUID();
    this.polyLineRectangle = new OGRectangle(ogid);

    const mergedOptions = { ...this.options, ...options, ogid };
    this.setConfig(mergedOptions);

    this.generateGeometry();
  }

  setConfig(options: RectangeOptions) {
    this.options = options;
    console.log("Rectangle options set:", this.options);
    const { breadth, width, center } = this.options;
    this.polyLineRectangle.set_config(
      center,
      width,
      breadth
    );
  }

  getConfig() {
    return this.options;
  }

  generateGeometry() {
    this.polyLineRectangle.generate_points();
    const bufRaw = this.polyLineRectangle.get_points();
    const bufFlush = JSON.parse(bufRaw);
    const line = new THREE.BufferGeometry().setFromPoints(bufFlush);
    const material = new THREE.LineBasicMaterial({ color: this.options.color });
    this.geometry = line;
    this.material = material;
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}