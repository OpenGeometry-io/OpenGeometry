import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import * as THREE from "three";

import {
  AnalyticSolid,
  type AnalyticAccuracy,
  type AnalyticPrimitiveOptions,
} from "./analytic-solid";

export interface ICylinderOptions {
  ogid?: string;
  center?: Vector3;
  radius?: number;
  height?: number;
  startAngle?: number;
  sweepAngle?: number;
  deflection?: number;
  accuracy?: AnalyticAccuracy;
  color?: THREE.ColorRepresentation;
}

interface NormalizedCylinderOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  height: number;
  startAngle: number;
  sweepAngle: number;
  deflection: number;
  accuracy?: AnalyticAccuracy;
  color: THREE.ColorRepresentation;
}

/** Authoritative Plane/Cylinder BRep with transient deflection tessellation. */
export class Cylinder extends AnalyticSolid {
  readonly options: Readonly<NormalizedCylinderOptions>;

  constructor(options: ICylinderOptions = {}) {
    const normalized = normalizeOptions(options);
    super(primitiveOptions(normalized));
    this.options = normalized;
    this.position.set(normalized.center.x, normalized.center.y, normalized.center.z);
  }

  get radius(): number { return this.options.radius; }
  get height(): number { return this.options.height; }
  get startAngle(): number { return this.options.startAngle; }
  get sweepAngle(): number { return this.options.sweepAngle; }

  getAnchor(): THREE.Vector3 {
    return this.position.clone();
  }
}

function normalizeOptions(options: ICylinderOptions): NormalizedCylinderOptions {
  return {
    ogid: options.ogid,
    center: options.center ?? new Vector3(0, 0, 0),
    radius: options.radius ?? 1,
    height: options.height ?? 1,
    startAngle: options.startAngle ?? 0,
    sweepAngle: options.sweepAngle ?? Math.PI * 2,
    deflection: options.deflection ?? 0.01,
    accuracy: options.accuracy,
    color: options.color ?? 0x00ff00,
  };
}

function primitiveOptions(options: NormalizedCylinderOptions): AnalyticPrimitiveOptions {
  const frame = {
    origin: [0, -options.height / 2, 0] as [number, number, number],
    x: [1, 0, 0] as [number, number, number],
    y: [0, 0, -1] as [number, number, number],
    z: [0, 1, 0] as [number, number, number],
  };
  const common = {
    ogid: options.ogid,
    frame,
    accuracy: options.accuracy,
    deflection: options.deflection,
    color: options.color,
    radius: options.radius,
    height: options.height,
  };
  return Math.abs(Math.abs(options.sweepAngle) - Math.PI * 2) <= 64 * Number.EPSILON
    ? { ...common, kind: "cylinder" }
    : {
        ...common,
        kind: "cylinderSector",
        startAngle: options.startAngle,
        sweepAngle: options.sweepAngle,
      };
}
