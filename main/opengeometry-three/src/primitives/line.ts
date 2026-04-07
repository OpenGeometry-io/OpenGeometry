import { OGLine, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';
import {
  clonePlacement,
  createParametricEditCapabilities,
} from "../editor";
import { createFreeformGeometry } from "../freeform";

/**
 * Construction options for a line segment wrapper.
 */
export interface ILineOptions {
  ogid?: string;
  start: Vector3;
  end: Vector3;
  color: number;
  fatLines?: boolean;
  width?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Placement updates accepted by `Line`.
 */
export interface LinePlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Partial config payload accepted by `Line.setConfig(...)`.
 */
export type LineConfigUpdate = Partial<
  Omit<ILineOptions, "translation" | "rotation" | "scale">
>;

/**
 * Alias for `Line` placement updates.
 */
export type LinePlacementUpdate = LinePlacementOptions;

/**
 * Offset result returned by `Line.offset(...)`.
 */
export interface ILineOffsetResult {
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

/**
 * Simple Line defined by Two Points
 */
export class Line extends THREE.Line {
  ogid: string;
  options: ILineOptions = {
    start: new Vector3(0, 0, 0.5),
    end: new Vector3(1, 0, 0.5),
    color: 0x000000,
    fatLines: false,
    width: 20,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };

  private line: OGLine;
  private fatLine: Line2 | null = null;

  set color(color: number) {
    this.options.color = color;
    if (this.material instanceof THREE.LineBasicMaterial) {
      this.material.color.set(color);
    }
    if (this.fatLine && this.fatLine.material instanceof LineMaterial) {
      this.fatLine.material.color.set(color);
    }
  }

  constructor(options?: ILineOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.line = new OGLine(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      start: this.options.start.clone(),
      end: this.options.end.clone(),
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
      throw new Error("Options are not defined for Line");
    }
  }

  setConfig(options: LineConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged = "start" in options || "end" in options;
    const renderChanged =
      "color" in options ||
      "fatLines" in options ||
      "width" in options;

    this.options = nextOptions;

    if (geometryChanged) {
      this.line.set_config(
        nextOptions.start.clone(),
        nextOptions.end.clone()
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

  getAnchor() {
    const anchor = this.line.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setAnchor(anchor: Vector3) {
    this.line.set_anchor(anchor.clone());
    this.generateGeometry();
  }

  resetAnchor() {
    this.line.reset_anchor();
    this.generateGeometry();
  }

  setPlacement(placement: LinePlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.line.set_transform(
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

  getPlacement() {
    return clonePlacement({
      translation: this.options.translation,
      rotation: this.options.rotation,
      scale: this.options.scale,
    });
  }

  getEditCapabilities() {
    return createParametricEditCapabilities("line", "profile");
  }

  canConvertToFreeform() {
    return true;
  }

  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(this.line.get_local_brep_serialized(), {
      id,
      placement: this.getPlacement(),
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
    const bufferData = Array.from(this.line.get_geometry_buffer());

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
  }

  getDXF() {
    const dxfData = this.line.get_dxf_serialized();
    return dxfData;
  }

  getOffset(
    distance: number,
    acuteThresholdDegrees: number = 35.0,
    bevel: boolean = true
  ): ILineOffsetResult {
    const kernel = this.line as unknown as {
      get_offset_serialized?: OffsetKernelFn;
    };
    if (typeof kernel.get_offset_serialized !== "function") {
      throw new Error(
        "Offset API is not available in OGLine. Rebuild opengeometry wasm bindings."
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
