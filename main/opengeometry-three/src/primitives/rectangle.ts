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

  setConfig(options: IRectangleOptions) {
    this.validateOptions();

    // Render Config Update
    // Note: For properties that directly impact rendering (like color), we can update them immediately without regenerating geometry.
    this.options = { ...this.options, ...options };

    console.log("Updated Rectangle Config:", this.options);

    // Kernel Config Update
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
    
    if (this.options.fatLines) {
      this.material.visible = false;
      this.handleFatLines(bufferData);
    } else {
      this.material.visible = true;
      if (this.fatLine) {
        this.fatLine.visible = false;
      }
    }
  }

  private handleFatLines(bufferData: number[]) {
    if (!this.fatLine) {
        this.fatLine = new Line2(new LineGeometry(), new LineMaterial({ color: this.options.color, linewidth: this.options.lineWidth, resolution: new THREE.Vector2(window.innerWidth, window.innerHeight) }));
        this.add(this.fatLine);
      }

      const positions = [];
      for (let i = 0; i < bufferData.length; i += 3) {
        positions.push(bufferData[i], bufferData[i + 1], bufferData[i + 2]);
      }

      this.fatLine.geometry.setPositions(positions);
      (this.fatLine.material as LineMaterial).color.set(this.options.color);
      (this.fatLine.material as LineMaterial).linewidth = this.options.lineWidth ?? 1;
      (this.fatLine.material as LineMaterial).resolution.set(window.innerWidth, window.innerHeight);

      this.fatLine.visible = true;

      console.log("Fat lines enabled for Rectangle");
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
