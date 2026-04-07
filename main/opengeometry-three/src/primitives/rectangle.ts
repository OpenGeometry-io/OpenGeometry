import { OGRectangle, Vector3 } from "./../../../opengeometry/pkg/opengeometry";
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
 * Construction options for a rectangle profile wrapper.
 */
export interface IRectangleOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  breadth: number;
  color: number;
  fatLines?: boolean;
  lineWidth?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Placement updates accepted by `Rectangle`.
 */
export interface RectanglePlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

/**
 * Partial config payload accepted by `Rectangle.setConfig(...)`.
 */
export type RectangleConfigUpdate = Partial<
  Omit<IRectangleOptions, "translation" | "rotation" | "scale">
>;

/**
 * Alias for `Rectangle` placement updates.
 */
export type RectanglePlacementUpdate = RectanglePlacementOptions;

/**
 * Offset result returned by `Rectangle.offset(...)`.
 */
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

/**
 * Rectangle wrapper backed by the kernel OGRectangle primitive.
 */
export class Rectangle extends THREE.Line {
  ogid: string;
  options: IRectangleOptions = {
    center: new Vector3(0, 0, 0),
    width: 1,
    breadth: 1,
    color: 0x00ff00,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
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

    this.setConfig({
      center: this.options.center.clone(),
      width: this.options.width,
      breadth: this.options.breadth,
      color: this.options.color,
      fatLines: this.options.fatLines,
      lineWidth: this.options.lineWidth,
    });
    
    this.setPlacement({
      translation: this.options.translation?.clone(),
      rotation: this.options.rotation?.clone(),
      scale: this.options.scale?.clone(),
    });
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

  getAnchor() {
    const anchor = this.polyLineRectangle.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setPlacement(placement: RectanglePlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.polyLineRectangle.set_transform(
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

  getConfig() {
    return this.options;
  }

  getPlacement() {
    return clonePlacement({
      translation: this.options.translation,
      rotation: this.options.rotation,
      scale: this.options.scale,
    });
  }

  getEditCapabilities() {
    return createParametricEditCapabilities("rectangle", "profile");
  }

  canConvertToFreeform() {
    return true;
  }

  toFreeform(id: string = this.ogid) {
    if (!this.canConvertToFreeform()) {
      throw new Error("This entity cannot be converted to freeform.");
    }
    return createFreeformGeometry(
      this.polyLineRectangle.get_local_brep_serialized(),
      {
        id,
        placement: this.getPlacement(),
      }
    );
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
    const bufferData = Array.from(this.polyLineRectangle.get_geometry_buffer());

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
