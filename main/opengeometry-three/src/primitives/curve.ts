import * as THREE from "three";
import { OGCurve, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { getUUID } from "../utils/randomizer";

export interface ICurveOptions {
  ogid?: string;
  controlPoints: Vector3[];
  color: number;
}

export interface ICurveOffsetResult {
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

export class Curve extends THREE.Line {
  ogid: string;
  options: ICurveOptions = {
    controlPoints: [],
    color: 0x00aa55,
  };

  private curve: OGCurve;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
  }

  constructor(options?: ICurveOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.curve = new OGCurve(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig(this.options);
  }

  setConfig(options: ICurveOptions) {
    const { controlPoints } = options;
    this.curve.set_config(controlPoints.map((point) => point.clone()));

    this.options = { ...this.options, ...options };
    this.generateGeometry();
  }

  private generateGeometry() {
    this.geometry.dispose();

    const geometryData = this.curve.get_geometry_serialized();
    const bufferData = JSON.parse(geometryData);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(bufferData, 3)
    );

    this.geometry = geometry;
    this.material = new THREE.LineBasicMaterial({ color: this.options.color });
  }

  getOffset(
    distance: number,
    acuteThresholdDegrees: number = 35.0,
    bevel: boolean = true
  ): ICurveOffsetResult {
    const kernel = this.curve as unknown as {
      get_offset_serialized?: OffsetKernelFn;
    };
    if (typeof kernel.get_offset_serialized !== "function") {
      throw new Error(
        "Offset API is not available in OGCurve. Rebuild opengeometry wasm bindings."
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
}
