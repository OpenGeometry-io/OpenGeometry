import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

import { createParametricEditCapabilities } from "../editor";
import {
  AnalyticSolid,
  type AnalyticAccuracy,
  type AnalyticPrimitiveOptions,
} from "./analytic-solid";

export interface CuboidPlacementOptions {
  translation?: Vector3;
  rotation?: Vector3;
  scale?: Vector3;
}

export interface ICuboidOptions extends CuboidPlacementOptions {
  ogid?: string;
  center?: Vector3;
  width?: number;
  height?: number;
  depth?: number;
  color?: THREE.ColorRepresentation;
  deflection?: number;
  accuracy?: AnalyticAccuracy;
}

export type CuboidConfigUpdate = Partial<
  Omit<ICuboidOptions, "ogid" | "translation" | "rotation" | "scale">
>;
export type CuboidPlacementUpdate = CuboidPlacementOptions;

interface NormalizedCuboidOptions {
  ogid?: string;
  center: Vector3;
  width: number;
  height: number;
  depth: number;
  color: THREE.ColorRepresentation;
  deflection: number;
  accuracy?: AnalyticAccuracy;
  translation: Vector3;
  rotation: Vector3;
  scale: Vector3;
}

/** Authoritative six-plane BRep with transient tessellation. */
export class Cuboid extends AnalyticSolid {
  options: NormalizedCuboidOptions;

  constructor(options: ICuboidOptions = {}) {
    const normalized = normalizeOptions(options);
    super(primitiveOptions(normalized));
    this.options = normalized;
    this.applyPlacement();
  }

  set width(value: number) { this.setConfig({ width: value }); }
  get width(): number { return this.options.width; }
  set height(value: number) { this.setConfig({ height: value }); }
  get height(): number { return this.options.height; }
  set depth(value: number) { this.setConfig({ depth: value }); }
  get depth(): number { return this.options.depth; }
  override set color(value: THREE.ColorRepresentation) {
    this.options.color = value;
    this.surface.material.color.set(value);
    this.surface.material.needsUpdate = true;
  }
  override get color(): THREE.Color { return this.surface.material.color; }

  setConfig(update: CuboidConfigUpdate): void {
    const geometryChanged = ["center", "width", "height", "depth", "accuracy"]
      .some((key) => key in update);
    const next = {
      ...this.options,
      ...update,
      center: update.center?.clone() ?? this.options.center,
    };
    if (geometryChanged) this.replaceDefinition(primitiveOptions(next));
    else if (update.deflection !== undefined) this.retessellate(update.deflection);
    this.options = next;
    if (update.color !== undefined) this.color = update.color;
  }

  getConfig(): Readonly<NormalizedCuboidOptions> {
    return this.options;
  }

  setPlacement(update: CuboidPlacementUpdate): void {
    this.options.translation = update.translation?.clone() ?? this.options.translation;
    this.options.rotation = update.rotation?.clone() ?? this.options.rotation;
    this.options.scale = update.scale?.clone() ?? this.options.scale;
    this.applyPlacement();
  }

  setTransform(translation: Vector3, rotation: Vector3, scale: Vector3): void {
    this.setPlacement({ translation, rotation, scale });
  }

  setTranslation(translation: Vector3): void { this.setPlacement({ translation }); }
  setRotation(rotation: Vector3): void { this.setPlacement({ rotation }); }
  setScale(scale: Vector3): void { this.setPlacement({ scale }); }

  getPlacement(): Required<CuboidPlacementOptions> {
    return {
      translation: this.options.translation.clone(),
      rotation: this.options.rotation.clone(),
      scale: this.options.scale.clone(),
    };
  }

  getAnchor(): Vector3 {
    return this.options.center.clone();
  }

  getEditCapabilities() {
    return createParametricEditCapabilities("cuboid", "box");
  }

  canConvertToFreeform(): false { return false; }

  toFreeform(): never {
    throw new Error("UnsupportedAnalyticEdit: BRep v2 freeform conversion is not implemented");
  }

  cleanGeometry(): void { this.dispose(); }
  discardGeometry(): void { this.surface.geometry.dispose(); }

  private applyPlacement(): void {
    this.position.copy(this.options.translation);
    this.rotation.set(
      this.options.rotation.x,
      this.options.rotation.y,
      this.options.rotation.z,
    );
    this.scale.copy(this.options.scale);
    this.updateMatrix();
  }
}

function normalizeOptions(options: ICuboidOptions): NormalizedCuboidOptions {
  return {
    ogid: options.ogid,
    center: options.center?.clone() ?? new Vector3(0, 0, 0),
    width: options.width ?? 1,
    height: options.height ?? 1,
    depth: options.depth ?? 1,
    color: options.color ?? 0x00ff00,
    deflection: options.deflection ?? 0.01,
    accuracy: options.accuracy,
    translation: options.translation?.clone() ?? new Vector3(0, 0, 0),
    rotation: options.rotation?.clone() ?? new Vector3(0, 0, 0),
    scale: options.scale?.clone() ?? new Vector3(1, 1, 1),
  };
}

function primitiveOptions(options: NormalizedCuboidOptions): AnalyticPrimitiveOptions {
  return {
    kind: "cuboid",
    ogid: options.ogid,
    width: options.width,
    depth: options.depth,
    height: options.height,
    frame: {
      origin: [
        options.center.x - options.width / 2,
        options.center.y - options.height / 2,
        options.center.z + options.depth / 2,
      ],
      x: [1, 0, 0],
      y: [0, 0, -1],
      z: [0, 1, 0],
    },
    accuracy: options.accuracy,
    deflection: options.deflection,
    color: options.color,
  };
}
