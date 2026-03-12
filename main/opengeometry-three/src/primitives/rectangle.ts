import { OGRectangle, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';

export interface IRectangleOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  breadth: number;
  color: number;
  fatLines?: boolean;
  lineWidth?: number;
}

export type RectangleConfigUpdate = Partial<IRectangleOptions>;

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
  private fatLine: Line2 | null = null;

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
    if (this.fatLine && this.fatLine.material instanceof LineMaterial) {
      this.fatLine.material.color.set(color);
    }
  }

  // set lineWidth(lineWidth: number) {
  //   this.options.lineWidth = lineWidth;
  //   if (this.material instanceof THREE.LineBasicMaterial) {
  //     (this.material as THREE.LineBasicMaterial).linewidth = lineWidth;
  //   }
  // }

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

  setConfig(options: RectangleConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "center" in options ||
      "width" in options ||
      "breadth" in options;
    const renderChanged =
      "color" in options ||
      "fatLines" in options ||
      "lineWidth" in options;

    this.options = nextOptions;

    if (geometryChanged) {
      this.polyLineRectangle.set_config(
        nextOptions.center.clone(),
        nextOptions.width,
        nextOptions.breadth,
      );
      this.generateGeometry();
      return;
    }

    if (renderChanged) {
      this.updateRenderStyle();
    }
  }

  getConfig() {
    return this.options;
  }

  private getCurrentPositions() {
    const attribute = this.geometry.getAttribute("position");
    if (!attribute || attribute.itemSize !== 3) {
      return [];
    }

    const positions = [];
    for (let index = 0; index < attribute.count; index += 1) {
      positions.push(
        attribute.getX(index),
        attribute.getY(index),
        attribute.getZ(index)
      );
    }

    return positions;
  }

  private updateRenderStyle(bufferData?: number[]) {
    const positions = bufferData ?? this.getCurrentPositions();

    if (this.options.fatLines) {
      if (!this.fatLine) {
        this.fatLine = new Line2(
          new LineGeometry(),
          new LineMaterial({
            color: this.options.color,
            linewidth: this.options.lineWidth,
            resolution: new THREE.Vector2(window.innerWidth, window.innerHeight),
          })
        );
        this.add(this.fatLine);
      }

      this.fatLine.geometry.setPositions(positions);
      (this.fatLine.material as LineMaterial).color.set(this.options.color);
      (this.fatLine.material as LineMaterial).linewidth = this.options.lineWidth ?? 1;
      (this.fatLine.material as LineMaterial).resolution.set(
        window.innerWidth,
        window.innerHeight
      );
      this.fatLine.visible = true;
    } else if (this.fatLine) {
      this.fatLine.visible = false;
    }

    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(this.options.color);
      this.material.visible = !this.options.fatLines;
    }
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
    this.updateRenderStyle(bufferData);
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
