import { OGRectangle, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";

export interface IRectangleOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  breadth: number;
  color: number;
}

export interface IRectangleOffsetResult {
  points: Vector3[];
  beveledVertexIndices: number[];
  isClosed: boolean;
}

type OffsetKernelOutput = {
  points: Array<{ x: number; y: number; z: number }>;
  beveled_vertex_indices: number[];
  is_closed: boolean;
};

/* eslint-disable no-unused-vars */
type OffsetKernelFn = (
  distance: number,
  acuteThresholdDegrees: number,
  bevel: boolean
) => string;
/* eslint-enable no-unused-vars */

export class Rectangle extends THREE.Line {
  ogid: string;
  options: IRectangleOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    breadth: 1,
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
  constructor(options?: IRectangleOptions) {
    super();
    this.ogid = options?.ogid ?? getUUID();
    this.polyLineRectangle = new OGRectangle(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig(this.options);
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Rectangle");
    }
  }

  setConfig(options: IRectangleOptions) {
    this.validateOptions();

    const { width, breadth, center } = options;
    this.polyLineRectangle.set_config(
      center.clone(),
      width,
      breadth,
    );

    this.generateGeometry();
  }

  getConfig() {
    return this.options;
  }

  private generateGeometry() {
    this.discardGeometry();

    this.polyLineRectangle.generate_geometry();
    const geometryData = this.polyLineRectangle.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: this.options.color });
  }

  getBrep() {
    const brepData = this.polyLineRectangle.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for Rectangle");
    }
    return JSON.parse(brepData);
  }

  getOffset(
    distance: number,
    acuteThresholdDegrees: number = 35.0,
    bevel: boolean = true
  ): IRectangleOffsetResult {
    const kernel = this.polyLineRectangle as unknown as {
      get_offset_serialized?: OffsetKernelFn;
    };
    if (typeof kernel.get_offset_serialized !== "function") {
      throw new Error(
        "Offset API is not available in OGRectangle. Rebuild opengeometry wasm bindings."
      );
    }

    const serialized = kernel.get_offset_serialized(
      distance,
      acuteThresholdDegrees,
      bevel
    );
    const parsed = JSON.parse(serialized) as OffsetKernelOutput;

    return {
      points: parsed.points.map((point) => new Vector3(point.x, point.y, point.z)),
      beveledVertexIndices: parsed.beveled_vertex_indices ?? [],
      isClosed: Boolean(parsed.is_closed),
    };
  }

  discardGeometry() {
    this.geometry.dispose();
  }
}
