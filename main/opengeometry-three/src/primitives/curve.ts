import * as THREE from "three";
import { OGCurve, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { getUUID } from "../utils/randomizer";

export interface ICurveOptions {
  ogid?: string;
  controlPoints: Vector3[];
  color: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export interface CurvePlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export type CurveConfigUpdate = Partial<
  Omit<ICurveOptions, "translation" | "rotation" | "scale">
>;
export type CurvePlacementUpdate = CurvePlacementOptions;

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
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
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

    this.setConfig({
      controlPoints: this.options.controlPoints.map((point) => point.clone()),
      color: this.options.color,
    });
    this.setPlacement({
      translation: this.options.translation?.clone(),
      rotation: this.options.rotation?.clone(),
      scale: this.options.scale?.clone(),
    });
  }

  setConfig(options: CurveConfigUpdate) {
    const nextOptions = { ...this.options, ...options };
    const geometryChanged = "controlPoints" in options;
    const colorChanged = "color" in options;

    this.options = nextOptions;

    if (geometryChanged) {
      this.curve.set_config(
        nextOptions.controlPoints.map((point) => point.clone())
      );
      this.generateGeometry();
      return;
    }

    if (colorChanged && this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(this.options.color);
    }
  }

  getAnchor() {
    const anchor = this.curve.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setAnchor(anchor: Vector3) {
    this.curve.set_anchor(anchor.clone());
    this.generateGeometry();
  }

  resetAnchor() {
    this.curve.reset_anchor();
    this.generateGeometry();
  }

  setPlacement(placement: CurvePlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.curve.set_transform(
      this.options.translation?.clone() ?? new Vector3(0, 0, 0),
      this.options.rotation?.clone() ?? new Vector3(0, 0, 0),
      this.options.scale?.clone() ?? new Vector3(1, 1, 1)
    );
    this.generateGeometry();
  }

  setTransform(translation: Vector3, rotation: Vector3, scale: Vector3) {
    this.setPlacement({
      translation,
      rotation,
      scale,
    });
  }

  setTranslation(translation: Vector3) {
    this.setPlacement({ translation });
  }

  setRotation(rotation: Vector3) {
    this.setPlacement({ rotation });
  }

  setScale(scale: Vector3) {
    this.setPlacement({ scale });
  }

  private generateGeometry() {
    const bufferData = Array.from(this.curve.get_geometry_buffer());

    this.writePositionsToGeometry(this.geometry, bufferData);
    this.geometry.computeBoundingBox();
    this.geometry.computeBoundingSphere();

    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(this.options.color);
    } else {
      if (Array.isArray(this.material)) {
        this.material.forEach((material) => material.dispose());
      } else {
        this.material.dispose();
      }

      this.material = new THREE.LineBasicMaterial({ color: this.options.color });
    }
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

  private writePositionsToGeometry(
    geometry: THREE.BufferGeometry,
    positions: number[]
  ) {
    const existing = geometry.getAttribute("position");
    if (
      !(existing instanceof THREE.BufferAttribute) ||
      existing.itemSize !== 3 ||
      existing.count !== positions.length / 3
    ) {
      geometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(positions, 3)
      );
      return;
    }

    const array = existing.array as Float32Array;
    array.set(positions);
    existing.needsUpdate = true;
  }
}
