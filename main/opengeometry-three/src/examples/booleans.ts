import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import {
  BooleanExecutionOptions,
  booleanIntersection,
  booleanSubtraction,
  booleanUnion,
} from "../operations/boolean";
import { AnalyticSolid } from "../shapes/analytic-solid";

export type BooleanExampleOperation = "union" | "intersection" | "subtraction";
export type BooleanExampleMode = "solid" | "polygon" | "extruded";

export interface BooleanExampleBuildOptions extends BooleanExecutionOptions {
  mode: BooleanExampleMode;
  sphereCenter?: Vector3;
  polygonOffset?: Vector3;
  extrudedOffset?: Vector3;
}

export interface BooleanExampleBuildResult {
  group: THREE.Group;
  lhs: THREE.Object3D;
  rhs: THREE.Object3D;
  result: AnalyticSolid;
  title: string;
  description: string;
}

/**
 * Builds a complete operand/result scene group for a boolean example page.
 */
export function createBooleanExample(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
): BooleanExampleBuildResult {
  const preset = createPreset(operation, options);

  const execute = getBooleanExecutor(operation);
  const result = execute(preset.lhsOperand, preset.rhsOperand, options);

  const group = new THREE.Group();
  group.add(preset.lhsVisual);
  group.add(preset.rhsVisual);
  group.add(result);

  return {
    group,
    lhs: preset.lhsVisual,
    rhs: preset.rhsVisual,
    result,
    title: preset.title,
    description: preset.description,
  };
}

function createPreset(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
) {
  switch (options.mode) {
    case "solid":
      return createSolidPreset(operation, options);
    case "polygon":
      return createPolygonPreset(operation, options);
    case "extruded":
      return createExtrudedPreset(operation, options);
  }
}

/**
 * Builds the solid-operand preset used by the example pages.
 */
function createSolidPreset(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
) {
  const base = new AnalyticSolid({
    kind: "sphere",
    radius: 1,
    color: 0x60a5fa,
    deflection: 0.01,
  });
  base.position.set(-0.45, 1, 0);

  const tool = new AnalyticSolid({
    kind: "sphere",
    radius: operation === "intersection" ? 0.95 : 0.85,
    color: 0xf97316,
    deflection: 0.01,
  });
  const center = options.sphereCenter ?? new Vector3(0.45, 1, 0.1);
  tool.position.set(center.x, center.y, center.z);

  base.outline = options.outline ?? true;
  tool.outline = options.outline ?? true;

  return {
    title: `${capitalize(operation)} Analytic Spheres`,
    description: "Two exact spheres exercise the in-house analytic boolean path and retain face ancestry.",
    lhsOperand: base,
    rhsOperand: tool,
    lhsVisual: base,
    rhsVisual: tool,
  };
}

/**
 * Builds the thin planar-profile solid preset used by the example pages.
 */
function createPolygonPreset(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
) {
  const polygonOffset = options.polygonOffset ?? new Vector3(0.0, 0.0, 0.0);
  const lhsVertices = [
    new Vector3(-1.8, 0.0, -0.8),
    new Vector3(0.4, 0.0, -0.8),
    new Vector3(0.4, 0.0, 1.0),
    new Vector3(-1.8, 0.0, 1.0),
  ];
  const rhsVertices = translateVertices(
    operation === "subtraction"
      ? [
          new Vector3(-0.55, 0.0, -0.35),
          new Vector3(1.2, 0.0, -0.35),
          new Vector3(1.2, 0.0, 0.65),
          new Vector3(-0.55, 0.0, 0.65),
        ]
      : [
          new Vector3(-0.4, 0.0, -1.1),
          new Vector3(1.55, 0.0, -1.1),
          new Vector3(1.55, 0.0, 0.7),
          new Vector3(-0.4, 0.0, 0.7),
        ],
    polygonOffset
  );

  const lhs = new AnalyticSolid({
    kind: "linearExtrusion",
    outer: lhsVertices.map((point) => [point.x, -point.z]),
    holes: [],
    height: 0.12,
    color: 0x60a5fa,
    deflection: 0.01,
  });
  const rhs = new AnalyticSolid({
    kind: "linearExtrusion",
    outer: rhsVertices.map((point) => [point.x, -point.z]),
    holes: [],
    height: 0.12,
    color: 0xf97316,
    deflection: 0.01,
  });

  lhs.outline = options.outline ?? true;
  rhs.outline = options.outline ?? true;

  return {
    title: `${capitalize(operation)} Planar Profile Solids`,
    description: "Two authoritative line-profile extrusions exercise planar imprint, classify, select, and sew.",
    lhsOperand: lhs,
    rhsOperand: rhs,
    lhsVisual: lhs,
    rhsVisual: rhs,
  };
}

/**
 * Builds a strict-v2 line-profile extrusion preset with a coextensive slab.
 */
function createExtrudedPreset(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
) {
  const extrudedOffset = options.extrudedOffset ?? new Vector3(0.0, 0.0, 0.0);

  const wall = new AnalyticSolid({
    kind: "linearExtrusion",
    outer: [[-2.2, -0.18], [2.2, -0.18], [2.2, 0.18], [-2.2, 0.18]],
    holes: [],
    height: 2.8,
    color: 0x60a5fa,
    deflection: 0.01,
  });
  const opening = new AnalyticSolid({
    kind: "linearExtrusion",
    outer: [[-0.7, -0.34], [0.9, -0.34], [0.9, 0.34], [-0.7, 0.34]],
    holes: [],
    height: 2.8,
    color: 0xf97316,
    deflection: 0.01,
  });
  opening.position.set(extrudedOffset.x, 0, extrudedOffset.z);

  wall.outline = options.outline ?? true;
  opening.outline = options.outline ?? true;

  return {
    title: `${capitalize(operation)} Extruded Solid`,
    description:
      "Two coextensive authoritative line-profile extrusions use the in-house planar arrangement.",
    lhsOperand: wall,
    rhsOperand: opening,
    lhsVisual: wall,
    rhsVisual: opening,
  };
}

/**
 * Selects the exported boolean helper that matches the current example mode.
 */
function getBooleanExecutor(operation: BooleanExampleOperation) {
  switch (operation) {
    case "union":
      return booleanUnion;
    case "intersection":
      return booleanIntersection;
    case "subtraction":
      return booleanSubtraction;
  }
}

/**
 * Formats the operation name for example titles.
 */
function capitalize(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

/**
 * Applies a uniform translation to the polygon operand so examples can move it
 * interactively without mutating the base preset coordinates.
 */
function translateVertices(vertices: Vector3[], offset: Vector3) {
  return vertices.map(
    (vertex) =>
      new Vector3(
        vertex.x + offset.x,
        vertex.y + offset.y,
        vertex.z + offset.z
      )
  );
}
