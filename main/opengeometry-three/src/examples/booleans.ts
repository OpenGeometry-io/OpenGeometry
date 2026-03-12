import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";

import {
  BooleanExecutionOptions,
  BooleanResult,
  booleanIntersection,
  booleanSubtraction,
  booleanUnion,
} from "../operations/boolean";
import { Cuboid } from "../shapes/cuboid";
import { Polygon } from "../shapes/polygon";
import { Sphere } from "../shapes/sphere";

export type BooleanExampleOperation = "union" | "intersection" | "subtraction";
export type BooleanExampleMode = "solid" | "polygon";

export interface BooleanExampleBuildOptions extends BooleanExecutionOptions {
  mode: BooleanExampleMode;
  sphereCenter?: Vector3;
  polygonOffset?: Vector3;
}

export interface BooleanExampleBuildResult {
  group: THREE.Group;
  lhs: THREE.Object3D;
  rhs: THREE.Object3D;
  result: BooleanResult;
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
  const preset = options.mode === "solid"
    ? createSolidPreset(operation, options)
    : createPolygonPreset(operation, options);

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

/**
 * Builds the solid-operand preset used by the example pages.
 */
function createSolidPreset(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
) {
  const outlineWidth = options.outlineWidth ?? 3;
  const base = new Cuboid({
    center: new Vector3(-0.1, 0.0, 0.0),
    width: 2.1,
    height: 1.4,
    depth: 1.6,
    color: 0x60a5fa,
    fatOutlines: options.fatOutlines ?? true,
    outlineWidth,
  });

  const tool = new Sphere({
    center: options.sphereCenter ?? new Vector3(0.45, 0.12, 0.18),
    radius: operation === "intersection" ? 0.88 : 0.78,
    widthSegments: 22,
    heightSegments: 16,
    color: 0xf97316,
    fatOutlines: options.fatOutlines ?? true,
    outlineWidth,
  });

  base.outline = options.outline ?? true;
  tool.outline = options.outline ?? true;

  return {
    title: `${capitalize(operation)} Solid`,
    description: "Cuboid and sphere overlap to demonstrate the 3D solid boolean path.",
    lhsOperand: base,
    rhsOperand: tool,
    lhsVisual: base,
    rhsVisual: tool,
  };
}

/**
 * Builds the coplanar polygon preset used by the example pages.
 */
function createPolygonPreset(
  operation: BooleanExampleOperation,
  options: BooleanExampleBuildOptions
) {
  const outlineWidth = options.outlineWidth ?? 3;
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

  const lhs = new Polygon({
    vertices: lhsVertices,
    color: 0x60a5fa,
    fatOutlines: options.fatOutlines ?? true,
    outlineWidth,
  });
  const rhs = new Polygon({
    vertices: rhsVertices,
    color: 0xf97316,
    fatOutlines: options.fatOutlines ?? true,
    outlineWidth,
  });

  lhs.outline = options.outline ?? true;
  rhs.outline = options.outline ?? true;

  return {
    title: `${capitalize(operation)} Polygon`,
    description: "Two coplanar polygon faces exercise the planar boolean path.",
    lhsOperand: lhs,
    rhsOperand: rhs,
    lhsVisual: lhs,
    rhsVisual: rhs,
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
