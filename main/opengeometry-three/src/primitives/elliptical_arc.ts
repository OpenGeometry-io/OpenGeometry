import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import { AnalyticCurve, type AnalyticCurveStats } from "./analytic-curve";
import type { AnalyticAccuracy, AnalyticFrame } from "../shapes/analytic-solid";

export interface IEllipticalArcOptions {
  ogid?: string;
  center?: Vector3;
  majorRadius?: number;
  minorRadius?: number;
  startAngle?: number;
  sweepAngle?: number;
  deflection?: number;
  accuracy?: AnalyticAccuracy;
  color?: THREE.ColorRepresentation;
}

export type EllipticalArcConfigUpdate = Partial<Omit<IEllipticalArcOptions, "ogid">>;

interface NormalizedEllipticalArcOptions {
  ogid?: string;
  center: Vector3;
  majorRadius: number;
  minorRadius: number;
  startAngle: number;
  sweepAngle: number;
  deflection: number;
  accuracy?: AnalyticAccuracy;
  color: THREE.ColorRepresentation;
}

/** Authoritative elliptical edge with transient deflection tessellation. */
export class EllipticalArc extends AnalyticCurve {
  options: Readonly<NormalizedEllipticalArcOptions>;

  constructor(options: IEllipticalArcOptions = {}) {
    const normalized = normalizeOptions(options);
    super({
      kind: "ellipticalArc",
      ogid: normalized.ogid,
      frame: curveFrame(normalized.center),
      accuracy: normalized.accuracy,
      majorRadius: normalized.majorRadius,
      minorRadius: normalized.minorRadius,
      startAngle: normalized.startAngle,
      sweepAngle: normalized.sweepAngle,
      deflection: normalized.deflection,
      color: normalized.color,
    });
    this.options = normalized;
  }

  getAnchor(): Vector3 {
    return this.options.center.clone();
  }

  getConfig(): Readonly<NormalizedEllipticalArcOptions> {
    return this.options;
  }

  setConfig(update: EllipticalArcConfigUpdate): AnalyticCurveStats {
    const next = normalizeOptions({ ...this.options, ...update, ogid: this.ogid });
    const statistics = this.replaceDefinition({
      kind: "ellipticalArc",
      ogid: this.ogid,
      frame: curveFrame(next.center),
      accuracy: next.accuracy,
      majorRadius: next.majorRadius,
      minorRadius: next.minorRadius,
      startAngle: next.startAngle,
      sweepAngle: next.sweepAngle,
      deflection: next.deflection,
      color: next.color,
    });
    this.options = next;
    return statistics;
  }

  getBrep(): Record<string, unknown> {
    return this.getBrepData();
  }

  retessellate(deflection: number): AnalyticCurveStats {
    return super.retessellate(deflection);
  }
}

function normalizeOptions(options: IEllipticalArcOptions): NormalizedEllipticalArcOptions {
  return {
    ogid: options.ogid,
    center: options.center ?? new Vector3(0, 0, 0),
    majorRadius: options.majorRadius ?? 1,
    minorRadius: options.minorRadius ?? 0.5,
    startAngle: options.startAngle ?? 0,
    sweepAngle: options.sweepAngle ?? Math.PI * 2,
    deflection: options.deflection ?? 0.01,
    accuracy: options.accuracy,
    color: options.color ?? 0x00ff00,
  };
}

function curveFrame(center: Vector3): AnalyticFrame {
  return {
    origin: [center.x, center.y, center.z],
    x: [1, 0, 0],
    y: [0, 0, -1],
    z: [0, 1, 0],
  };
}
