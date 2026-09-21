import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import { AnalyticCurve, type AnalyticCurveStats } from "./analytic-curve";
import type { AnalyticAccuracy, AnalyticFrame } from "../shapes/analytic-solid";

export interface IArcOptions {
  ogid?: string;
  center?: Vector3;
  radius?: number;
  startAngle?: number;
  sweepAngle?: number;
  deflection?: number;
  accuracy?: AnalyticAccuracy;
  color?: THREE.ColorRepresentation;
}

export type ArcConfigUpdate = Partial<Omit<IArcOptions, "ogid">>;

interface NormalizedArcOptions {
  ogid?: string;
  center: Vector3;
  radius: number;
  startAngle: number;
  sweepAngle: number;
  deflection: number;
  accuracy?: AnalyticAccuracy;
  color: THREE.ColorRepresentation;
}

/** Authoritative circular edge with transient deflection tessellation. */
export class Arc extends AnalyticCurve {
  options: Readonly<NormalizedArcOptions>;

  constructor(options: IArcOptions = {}) {
    const normalized = normalizeOptions(options);
    super({
      kind: "arc",
      ogid: normalized.ogid,
      frame: curveFrame(normalized.center),
      accuracy: normalized.accuracy,
      radius: normalized.radius,
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

  getConfig(): Readonly<NormalizedArcOptions> {
    return this.options;
  }

  setConfig(update: ArcConfigUpdate): AnalyticCurveStats {
    const next = normalizeOptions({ ...this.options, ...update, ogid: this.ogid });
    const statistics = this.replaceDefinition({
      kind: "arc",
      ogid: this.ogid,
      frame: curveFrame(next.center),
      accuracy: next.accuracy,
      radius: next.radius,
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

function normalizeOptions(options: IArcOptions): NormalizedArcOptions {
  return {
    ogid: options.ogid,
    center: options.center ?? new Vector3(0, 0, 0),
    radius: options.radius ?? 1,
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
