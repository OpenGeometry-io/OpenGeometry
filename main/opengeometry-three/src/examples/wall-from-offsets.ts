import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Cuboid } from "../shapes/cuboid";
import { Polyline } from "../primitives/polyline";
import { Polygon } from "../shapes/polygon";
import {
  BooleanError,
  CoincidentFacesError,
  CutterExceedsHostError,
  NonManifoldOutputError,
  OverlappingCuttersError,
} from "../operations/boolean-errors";

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
  /**
   * Number of doors to subtract from the extruded wall. Doors are placed at
   * evenly distributed centerline-segment midpoints. Default `0` (no cuts).
   */
  doorCount?: number;
  /**
   * Cutter overshoot in millimeters. The debt sheet's `#6a` symptom is at
   * 1mm; default 1mm so callers exercise the snap-tolerance regime by default
   * when cuts are enabled.
   */
  overshootMm?: number;
  /**
   * Wall extrusion height in meters. Default `2.6` to match the example HTML.
   */
  height?: number;
}

function midpoint(a: Vector3, b: Vector3): Vector3 {
  return new Vector3((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
}

function buildDoorCutters(
  centerlinePoints: Vector3[],
  count: number,
  overshootMeters: number,
  wallThickness: number,
  doorHeight: number
): Cuboid[] {
  const cutters: Cuboid[] = [];
  const wallDepth = wallThickness + 2 * overshootMeters;
  const doorWidth = 0.6;

  const segments = centerlinePoints.length - 1;
  const stride = Math.max(1, Math.floor(segments / count));

  for (let doorIdx = 0; doorIdx < count; doorIdx++) {
    const segmentIdx = Math.min(segments - 1, doorIdx * stride);
    const a = centerlinePoints[segmentIdx];
    const b = centerlinePoints[segmentIdx + 1];
    const mid = midpoint(a, b);

    cutters.push(
      new Cuboid({
        width: doorWidth,
        height: doorHeight,
        depth: wallDepth,
        center: new Vector3(0, 0, 0),
        translation: new Vector3(mid.x, doorHeight * 0.5 + 0.001, mid.z),
        color: 0xef4444,
      })
    );
  }

  return cutters;
}

/**
 * Renders a human-readable description for a typed `BooleanError` (or any
 * other thrown value). Used by the example HTML to surface kernel failures
 * in a status panel without losing the structured payload.
 */
export function describeWallSubtractError(error: unknown): string {
  if (error instanceof OverlappingCuttersError) {
    return `OverlappingCuttersError @ cutter #${error.cutterIndex} (other #${error.otherIndex}): ${error.details ?? error.message}`;
  }
  if (error instanceof CutterExceedsHostError) {
    return `CutterExceedsHostError: cutter overshoots ${error.axis} by ${error.overshoot.toExponential(3)}`;
  }
  if (error instanceof CoincidentFacesError) {
    return `CoincidentFacesError: ${error.details ?? error.message}`;
  }
  if (error instanceof NonManifoldOutputError) {
    const samples = error.edgeSamples?.length ?? 0;
    return `NonManifoldOutputError: ${samples} non-manifold edges sampled — ${error.details ?? error.message}`;
  }
  if (error instanceof BooleanError) {
    return `BooleanError (${error.reason}, phase ${error.phase}): ${error.message}`;
  }
  if (error instanceof Error) {
    return `Untyped error: ${error.message}`;
  }
  return `Untyped error: ${String(error)}`;
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

  let cutResult: { wall: THREE.Object3D; cutters: Cuboid[] } | null = null;
  let cutError: unknown = null;

  if ((options.doorCount ?? 0) > 0) {
    const overshootMeters = (options.overshootMm ?? 1) * 0.001;
    const wallSolid = wallPolygon.extrude(wallHeight);
    const cutters = buildDoorCutters(
      [
        new Vector3(-2.6, 0.0, -1.9),
        new Vector3(-1.2, 0.0, -1.0),
        new Vector3(-0.2, 0.0, 0.1),
        new Vector3(0.6, 0.0, 0.2),
        new Vector3(0.1, 0.0, 1.0),
        new Vector3(2.6, 0.0, 2.0),
      ],
      options.doorCount ?? 0,
      overshootMeters,
      wallThickness,
      2.1
    );

    try {
      const cutWall = wallSolid.subtract(cutters, {
        color: 0x3b82f6,
        transparent: false,
        opacity: 1,
      });
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
