import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Polyline } from "../primitives/polyline";
import { AnalyticSolid } from "../shapes/analytic-solid";
import { Polygon } from "../shapes/polygon";

const EPSILON = 1.0e-9;

function areClose(a: Vector3, b: Vector3): boolean {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const dz = a.z - b.z;
  return (dx * dx + dy * dy + dz * dz) <= EPSILON * EPSILON;
}

function buildWallOutline(left: Vector3[], right: Vector3[]): Vector3[] {
  if (left.length === 0 || right.length === 0) {
    return [];
  }

  const outline = [...left.map((point) => point.clone())];
  const rightReversed = [...right.map((point) => point.clone())].reverse();
  outline.push(...rightReversed);

  if (outline.length > 2 && areClose(outline[0], outline[outline.length - 1])) {
    outline.pop();
  }

  return outline;
}

export interface WallFromOffsetsOptions {
  /** Subtract one exact coextensive through slot. Default `false`. */
  cutThroughSlot?: boolean;
  /** Width of the through slot along its centerline segment. Default `0.6`. */
  slotWidth?: number;
  /**
   * Wall extrusion height in meters. Default `2.6` to match the example HTML.
   */
  height?: number;
}

function midpoint(a: Vector3, b: Vector3): Vector3 {
  return new Vector3((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
}

function buildThroughSlot(
  centerlinePoints: Vector3[],
  wallThickness: number,
  height: number,
  width: number,
): AnalyticSolid {
  const a = centerlinePoints[2];
  const b = centerlinePoints[3];
  const mid = midpoint(a, b);
  const length = Math.hypot(b.x - a.x, b.z - a.z);
  const tangent = [(b.x - a.x) / length, (b.z - a.z) / length];
  const normal = [-tangent[1], tangent[0]];
  const halfWidth = width * 0.5;
  const halfDepth = wallThickness * 0.5 + 0.1;
  const point = (along: number, across: number): [number, number] => [
    mid.x + tangent[0] * along + normal[0] * across,
    -(mid.z + tangent[1] * along + normal[1] * across),
  ];
  return new AnalyticSolid({
    kind: "linearExtrusion",
    outer: [
      point(-halfWidth, -halfDepth),
      point(halfWidth, -halfDepth),
      point(halfWidth, halfDepth),
      point(-halfWidth, halfDepth),
    ],
    holes: [],
    height,
    color: 0xef4444,
    deflection: 0.01,
  });
}

/**
 * Renders a human-readable description for an analytic geometry error (or any
 * other thrown value). Used by the example HTML to surface kernel failures
 * in a status panel without losing the structured payload.
 */
export function describeWallSubtractError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export function createWallFromOffsetsExample(
  scene: THREE.Scene,
  options: WallFromOffsetsOptions = {}
) {
  const centerline = new Polyline({
    points: [
      new Vector3(-2.6, 0.0, -1.9),
      new Vector3(-1.2, 0.0, -1.0),
      new Vector3(-0.2, 0.0, 0.1),
      new Vector3(0.6, 0.0, 0.2),
      new Vector3(0.1, 0.0, 1.0),
      new Vector3(2.6, 0.0, 2.0),
    ],
    color: 0x1f2937,
  });

  const wallThickness = 0.45;
  const half = wallThickness * 0.5;
  const acuteThreshold = 35.0;
  const bevel = true;
  const wallHeight = options.height ?? 2.6;

  const leftOffset = centerline.getOffset(half, acuteThreshold, bevel);
  const rightOffset = centerline.getOffset(-half, acuteThreshold, bevel);

  const leftOffsetPolyline = new Polyline({
    points: leftOffset.points,
    color: 0x22c55e,
  });

  const rightOffsetPolyline = new Polyline({
    points: rightOffset.points,
    color: 0xf97316,
  });

  const wallOutline = buildWallOutline(leftOffset.points, rightOffset.points);
  if (wallOutline.length < 3) {
    throw new Error("Failed to create wall polygon from offsets.");
  }

  const wallPolygon = new Polygon({
    vertices: wallOutline,
    color: 0x3b82f6,
  });

  wallPolygon.position.y = 0.01;

  scene.add(centerline);
  scene.add(leftOffsetPolyline);
  scene.add(rightOffsetPolyline);
  scene.add(wallPolygon);

  let cutResult: { wall: THREE.Object3D; cutters: AnalyticSolid[] } | null = null;
  let cutError: unknown = null;

  if (options.cutThroughSlot ?? false) {
    const wallSolid = new AnalyticSolid({
      kind: "linearExtrusion",
      outer: wallOutline.map((point) => [point.x, -point.z]),
      holes: [],
      height: wallHeight,
      color: 0x3b82f6,
      deflection: 0.01,
    });
    const cutter = buildThroughSlot(
      [
        new Vector3(-2.6, 0.0, -1.9),
        new Vector3(-1.2, 0.0, -1.0),
        new Vector3(-0.2, 0.0, 0.1),
        new Vector3(0.6, 0.0, 0.2),
        new Vector3(0.1, 0.0, 1.0),
        new Vector3(2.6, 0.0, 2.0),
      ],
      wallThickness,
      wallHeight,
      options.slotWidth ?? 0.6,
    );
    const cutters = [cutter];

    try {
      const cutWall = wallSolid.subtract(cutters, { color: 0x3b82f6 });
      scene.add(cutWall);
      cutResult = { wall: cutWall, cutters };
    } catch (error) {
      cutError = error;
      scene.add(wallSolid);
      cutResult = { wall: wallSolid, cutters };
    }
  }

  return {
    centerline,
    leftOffsetPolyline,
    rightOffsetPolyline,
    wallPolygon,
    leftOffset,
    rightOffset,
    cutResult,
    cutError,
  };
}
