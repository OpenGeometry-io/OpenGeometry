import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Line } from "../primitives/line";
import { Polyline } from "../primitives/polyline";
import { Rectangle } from "../primitives/rectangle";
import { Curve } from "../primitives/curve";

const EPSILON = 1.0e-9;

function areClose(a: Vector3, b: Vector3): boolean {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  const dz = a.z - b.z;
  return (dx * dx + dy * dy + dz * dz) <= EPSILON * EPSILON;
}

function toRenderableOffsetPoints(points: Vector3[], isClosed: boolean): Vector3[] {
  const output = points.map((point) => point.clone());
  if (!isClosed || output.length === 0) {
    return output;
  }

  if (!areClose(output[0], output[output.length - 1])) {
    output.push(output[0].clone());
  }

  return output;
}

export function createOffsetExample(scene: THREE.Scene) {
  const baseLine = new Line({
    start: new Vector3(-3.0, 0.0, -2.2),
    end: new Vector3(-0.7, 0.0, -1.1),
    color: 0x1f2937,
  });

  const lineOffset = baseLine.getOffset(0.35, 35.0, true);
  const lineOffsetPrimitive = new Line({
    start: lineOffset.points[0],
    end: lineOffset.points[1],
    color: 0xef4444,
  });

  const basePolyline = new Polyline({
    points: [
      new Vector3(-1.2, 0.0, -2.4),
      new Vector3(0.2, 0.0, -1.6),
      new Vector3(0.8, 0.0, -0.4),
      new Vector3(0.0, 0.0, -0.2),
      new Vector3(2.5, 0.0, 1.2),
    ],
    color: 0x0f172a,
  });

  const polylineOffset = basePolyline.getOffset(0.45, 90.0, true);
  const polylineOffsetPrimitive = new Polyline({
    points: polylineOffset.points,
    color: 0xdc2626,
  });

  const baseRectangle = new Rectangle({
    center: new Vector3(1.2, 0.0, -2.1),
    width: 1.6,
    breadth: 1.0,
    color: 0x1d4ed8,
  });

  const rectangleOffset = baseRectangle.getOffset(0.25, 40.0, true);
  const rectangleOffsetPrimitive = new Polyline({
    points: toRenderableOffsetPoints(rectangleOffset.points, rectangleOffset.isClosed),
    color: 0xf97316,
  });

  const baseCurve = new Curve({
    controlPoints: [
      new Vector3(-3.0, 0.0, 1.3),
      new Vector3(-2.0, 0.0, 1.7),
      new Vector3(-1.1, 0.0, 1.6),
      new Vector3(-0.4, 0.0, 2.0),
    ],
    color: 0x0ea5e9,
  });

  const curveOffset = baseCurve.getOffset(0.3, 45.0, true);
  const curveOffsetPrimitive = new Polyline({
    points: curveOffset.points,
    color: 0xbe123c,
  });

  scene.add(baseLine);
  scene.add(lineOffsetPrimitive);
  scene.add(basePolyline);
  scene.add(polylineOffsetPrimitive);
  scene.add(baseRectangle);
  scene.add(rectangleOffsetPrimitive);
  scene.add(baseCurve);
  scene.add(curveOffsetPrimitive);

  return {
    baseLine,
    lineOffsetPrimitive,
    basePolyline,
    polylineOffsetPrimitive,
    baseRectangle,
    rectangleOffsetPrimitive,
    baseCurve,
    curveOffsetPrimitive,
    polylineOffset,
    rectangleOffset,
    curveOffset,
  };
}
