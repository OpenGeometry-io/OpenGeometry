import * as THREE from "three";

import {
  AnalyticSolid,
  type AnalyticPolygonLoftAlignment,
} from "../shapes/analytic-solid";

export type PolygonLoftPoint = readonly [number, number, number];

export interface PolygonLoftOptions {
  alignment?: AnalyticPolygonLoftAlignment;
  deflection?: number;
  color?: THREE.ColorRepresentation;
}

/**
 * Creates a capped ruled solid between two planar polygon sections.
 * Sections must have equal vertex counts and produce planar side faces.
 */
export function loftPolygonSections(
  sections: readonly [readonly PolygonLoftPoint[], readonly PolygonLoftPoint[]],
  options: PolygonLoftOptions = {},
): AnalyticSolid {
  const [lower, upper] = sections;
  return new AnalyticSolid({
    kind: "polygonLoft",
    lower: copySection(lower, "lower"),
    upper: copySection(upper, "upper"),
    alignment: options.alignment ?? "auto",
    deflection: options.deflection,
    color: options.color,
  });
}

function copySection(
  section: readonly PolygonLoftPoint[],
  label: string,
): [number, number, number][] {
  if (section.length < 3) throw new Error(`${label} loft section requires at least three points`);
  return section.map((point) => {
    if (point.length !== 3 || !point.every(Number.isFinite)) {
      throw new Error(`${label} loft section points must contain three finite coordinates`);
    }
    return [point[0], point[1], point[2]];
  });
}
