import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

import { AnalyticSolid, type AnalyticAccuracy } from "./analytic-solid";

export interface ISphereOptions {
  ogid?: string;
  center?: Vector3;
  radius?: number;
  deflection?: number;
  accuracy?: AnalyticAccuracy;
  color?: THREE.ColorRepresentation;
}

interface NormalizedSphereOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  deflection: number;
  accuracy?: AnalyticAccuracy;
  color: THREE.ColorRepresentation;
}

/** Authoritative spherical BRep with explicit poles and transient deflection tessellation. */
export class Sphere extends AnalyticSolid {
  readonly options: Readonly<NormalizedSphereOptions>;

  constructor(options: ISphereOptions = {}) {
    const normalized = normalizeOptions(options);
    super({
      kind: "sphere",
      ogid: normalized.ogid,
      radius: normalized.radius,
      accuracy: normalized.accuracy,
      deflection: normalized.deflection,
      color: normalized.color,
    });
    this.options = normalized;
    this.position.set(normalized.center.x, normalized.center.y, normalized.center.z);
  }

  get radius(): number { return this.options.radius; }

  getAnchor(): THREE.Vector3 {
    return this.position.clone();
  }
}

function normalizeOptions(options: ISphereOptions): NormalizedSphereOptions {
  return {
    ogid: options.ogid,
    center: options.center ?? new Vector3(0, 0, 0),
    radius: options.radius ?? 1,
    deflection: options.deflection ?? 0.01,
    accuracy: options.accuracy,
    color: options.color ?? 0x00ff00,
  };
}
