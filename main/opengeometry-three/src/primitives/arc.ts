import { OGArc, Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";
import { getUUID } from "../utils/randomizer";
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';
// import { IArcOptions } from "../base-types";

export interface IArcOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  startAngle: number;
  endAngle: number;
  segments: number;
  color: number;
  fatLines?: boolean;
  width?: number;
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export interface ArcPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export type ArcConfigUpdate = Partial<
  Omit<IArcOptions, "translation" | "rotation" | "scale">
>;
export type ArcPlacementUpdate = ArcPlacementOptions;

export class Arc extends THREE.Line {
  ogid: string;
  options: IArcOptions = {
    center: new Vector3(0, 0, 0),
    radius: 3.5,
    startAngle: 0,
    endAngle: Math.PI * 2,
    segments: 32,
    color: 0x00ff00,
    fatLines: false,
    width: 20,
    translation: new Vector3(0, 0, 0),
    rotation: new Vector3(0, 0, 0),
    scale: new Vector3(1, 1, 1),
  };

  private arc: OGArc;
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

  constructor(options?: IArcOptions) {
    super();

    this.ogid = options?.ogid ?? getUUID();
    this.arc = new OGArc(this.ogid);

    this.options = { ...this.options, ...options };
    this.options.ogid = this.ogid;

    this.setConfig({
      center: this.options.center.clone(),
      radius: this.options.radius,
      startAngle: this.options.startAngle,
      endAngle: this.options.endAngle,
      segments: this.options.segments,
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
      throw new Error("Options are not defined for Circle Arc");
    }
  }

  setConfig(options: ArcConfigUpdate) {
    this.validateOptions();

    const nextOptions = { ...this.options, ...options };
    const geometryChanged =
      "center" in options ||
      "radius" in options ||
      "segments" in options ||
      "startAngle" in options ||
      "endAngle" in options;
    const renderChanged =
      "color" in options ||
      "fatLines" in options ||
      "width" in options;

    this.options = nextOptions;

    if (geometryChanged) {
      this.arc.set_config(
        nextOptions.center.clone(),
        nextOptions.radius,
        nextOptions.startAngle,
        nextOptions.endAngle,
        nextOptions.segments
      );

      this.generateGeometry();
      return;
    }

    if (renderChanged) {
      this.updateRenderStyle();
    }
  }

  getAnchor() {
    const anchor = this.arc.get_anchor();
    return new Vector3(anchor.x, anchor.y, anchor.z);
  }

  setPlacement(placement: ArcPlacementUpdate) {
    this.options.translation = placement.translation?.clone() ?? this.options.translation;
    this.options.rotation = placement.rotation?.clone() ?? this.options.rotation;
    this.options.scale = placement.scale?.clone() ?? this.options.scale;

    this.arc.set_transform(
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
    const bufferData = Array.from(this.arc.get_geometry_buffer());

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
    const brepData = this.arc.get_brep_serialized();
    if (!brepData) {
      throw new Error("Brep data is not available for Arc");
    }
    return JSON.parse(brepData);
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
