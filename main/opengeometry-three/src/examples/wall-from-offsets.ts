import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Polyline } from "../primitives/polyline";
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

export function createWallFromOffsetsExample(scene: THREE.Scene) {
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
  const acuteThreshold = 90.0;
  const bevel = true;

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

  return {
    centerline,
    leftOffsetPolyline,
    rightOffsetPolyline,
    wallPolygon,
    leftOffset,
    rightOffset,
  };
}
