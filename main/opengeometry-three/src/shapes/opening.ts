import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

import { createParametricEditCapabilities } from "../editor";
import { AnalyticSolid, type AnalyticAccuracy } from "./analytic-solid";
import {
  Cuboid,
  type CuboidConfigUpdate,
  type CuboidPlacementOptions,
  type ICuboidOptions,
} from "./cuboid";

export interface IOpeningOptions extends CuboidPlacementOptions {
  ogid?: string;
  center?: Vector3;
  width?: number;
  height?: number;
  depth?: number;
  color?: THREE.ColorRepresentation;
  deflection?: number;
  accuracy?: AnalyticAccuracy;
}

export type OpeningPlacementOptions = CuboidPlacementOptions;
export type OpeningConfigUpdate = CuboidConfigUpdate;
export type OpeningPlacementUpdate = OpeningPlacementOptions;

/** Strict BRep v2 subtractive box. */
export class Opening extends Cuboid {
  constructor(options: IOpeningOptions = {}) {
    const normalized: ICuboidOptions = {
      center: options.center ?? new Vector3(0, 0, 0),
      width: options.width ?? 1,
      height: options.height ?? 1,
      depth: options.depth ?? 0.2,
      color: options.color ?? 0xdad7cd,
      ...options,
    };
    super(normalized);
    this.surface.material.transparent = true;
    this.surface.material.opacity = 0;
    this.surface.material.depthWrite = false;
    this.surface.material.needsUpdate = true;
  }

  get dimensions() {
    return { width: this.width, height: this.height, depth: this.depth };
  }

  override getEditCapabilities() {
    return createParametricEditCapabilities("opening", "box");
  }

  subtractFrom(
    host: AnalyticSolid,
    options: { deflection?: number; color?: THREE.ColorRepresentation } = {},
  ): AnalyticSolid {
    if (!(host instanceof AnalyticSolid)) {
      throw new Error("Opening subtraction requires a strict analytic BRep v2 host");
    }
    return host.subtract([this], options);
  }
}
