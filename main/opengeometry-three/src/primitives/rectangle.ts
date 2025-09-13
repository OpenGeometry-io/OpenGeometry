import { OGRectangle, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { IRectangeOptions } from "../base-types";
import { BaseLinePrimitive } from "./base-line-primitive";

export class Rectangle extends BaseLinePrimitive {
  ogid: string;
  options: IRectangeOptions = {
    width: 1,
    breadth: 1,
    center: new Vector3(0, 0, 0),
    color: 0x00ff00,
  };

  private polyLineRectangle: OGRectangle;

  // set width(width: number) {
  //   this.options.width = width;
  //   this.polyLineRectangle.update_width(width);
  //   this.generateGeometry();
  // }

  // set breadth(breadth: number) {
  //   this.options.breadth = breadth;
  //   this.polyLineRectangle.update_breadth(breadth);
  //   this.generateGeometry();
  // }

  // set center(center: Vector3) {
  //   this.options.center = center;
  //   this.polyLineRectangle.update_center(center);
  //   this.generateGeometry();
  // }

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  // FINAL: This flow should be used for other primitives
  constructor(options?: IRectangeOptions) {
    super();

    const ogid = options?.ogid ?? getUUID();
    this.polyLineRectangle = new OGRectangle(ogid);

    // const mergedOptions = { ...this.options, ...options, ogid };
    if (options) {
      this.options = { ...this.options, ...options };
      this.ogid = options.ogid || getUUID();
    } else {
      this.ogid = getUUID();
    }

    this.setConfig();
    this.generateGeometry();
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Rectangle");
    }
  }

  setConfig() {
    this.validateOptions();

    const { width, breadth, center, color } = this.options;
    this.polyLineRectangle.set_config(
      center.clone() || new Vector3(0, 0, 0),
      width,
      breadth,
    );
  }

  getConfig() {
    return this.options;
  }

  generateGeometry() {
    this.polyLineRectangle.generate_geometry();
    const geometryData = this.polyLineRectangle.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);
    console.log(bufferData);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: 0x00ff00 });
  }

  getBrep() {
    const brepData = this.polyLineRectangle.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for Rectangle");
    }
    return JSON.parse(brepData);
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}