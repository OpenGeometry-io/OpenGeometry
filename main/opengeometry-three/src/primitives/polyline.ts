import * as THREE from 'three';
import { OGPolyline, Vector3 } from '../../../opengeometry/pkg/opengeometry';
import { getUUID } from "../utils/randomizer";
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';

export interface IPolylineOptions {
  ogid?: string;
  color: number;
  points: Vector3[];
  fatLines?: boolean;
  width?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export interface PolylinePlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export type PolylineConfigUpdate = Partial<
  Omit<IPolylineOptions, "translation" | "rotation" | "scale">
>;
export type PolylinePlacementUpdate = PolylinePlacementOptions;

export interface IOffsetResult {
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

export class Polyline extends THREE.Line {
  ogid: string;
  options: IPolylineOptions = {
    points: [],
    color: 0x00ff00,
    fatLines: false,
    width: 20,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };

  isClosed: boolean = false;

  private polyline: OGPolyline;
  private fatLine: Line2 | null = null;

  transformationMatrix: THREE.Matrix4 = new THREE.Matrix4();

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
    if (this.fatLine && this.fatLine.material instanceof LineMaterial) {
      this.fatLine.material.color.set(color);
    }
  }

  constructor(options?: IPolylineOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.polyline = new OGPolyline(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      points: this.options.points.map((point) => point.clone()),
      color: this.options.color,
      fatLines: this.options.fatLines,
      width: this.options.width,
    });
    this.setPlacement({
      translation: this.options.translation?.clone(),
      rotation: this.options.rotation?.clone(),
      scale: this.options.scale?.clone(),
    });
  }

  validateOptions() {
    if (!this.options) {
      throw new Error("Options are not defined for Polyline");
    }
  }

  setConfig(options: PolylineConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged = "points" in options;
    const renderChanged =
      "color" in options ||
      "fatLines" in options ||
      "width" in options;

    this.options = nextOptions;

    if (geometryChanged) {
      this.polyline.set_config(nextOptions.points.map((point) => point.clone()));
      this.generateGeometry();
      return;
    }

    if (renderChanged) {
      this.updateRenderStyle();
    }
  }

  getAnchor() {
    const anchor = this.polyline.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setAnchor(anchor: Vector3) {
    this.polyline.set_anchor(anchor.clone());
    this.generateGeometry();
  }

  resetAnchor() {
    this.polyline.reset_anchor();
    this.generateGeometry();
  }

  setPlacement(placement: PolylinePlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.polyline.set_transform(
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

  addPoint(point: Vector3) {
    if (!this.polyline) return;

    const { points } = this.options;
    points.push(point);

    if (this.options.points.length < 2) return;
    this.setConfig({
      points: this.options.points.map((currentPoint) => currentPoint.clone()),
    });
  }

  /**
   * Every time there are property changes, geometry needs to be discarded and regenerated.
   * This is to ensure that the geometry is always up-to-date with the current state.
   */
  discardGeometry() {
    this.geometry.dispose();
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
            linewidth: this.options.width,
            resolution: new THREE.Vector2(window.innerWidth, window.innerHeight),
          })
        );
        this.add(this.fatLine);
      }

      this.fatLine.geometry.setPositions(positions);
      (this.fatLine.material as LineMaterial).color.set(this.options.color);
      (this.fatLine.material as LineMaterial).linewidth = this.options.width ?? 5;
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
    const bufferData = Array.from(this.polyline.get_geometry_buffer());

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

    this.updateRenderStyle(bufferData);

    this.isClosed = this.polyline.is_closed();
  }

  getOffset(
    distance: number,
    acuteThresholdDegrees: number = 35.0,
    bevel: boolean = true
  ): IOffsetResult {
    const kernel = this.polyline as unknown as {
      get_offset_serialized?: OffsetKernelFn;
    };
    if (typeof kernel.get_offset_serialized !== "function") {
      throw new Error(
        "Offset API is not available in OGPolyline. Rebuild opengeometry wasm bindings."
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
