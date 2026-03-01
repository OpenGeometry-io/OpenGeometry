import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Arc } from "../primitives/arc";
import { Curve } from "../primitives/curve";
import { Line } from "../primitives/line";
import { Polyline } from "../primitives/polyline";
import { Rectangle } from "../primitives/rectangle";

/**
 * Adds a basic primitives showcase to the provided scene.
 */
export function createPrimitivesExample(scene: THREE.Scene) {
  const line = new Line({
    start: new Vector3(-3.0, 0.0, -2.0),
    end: new Vector3(-1.0, 0.0, -1.0),
    color: 0x111827,
  });

  const polyline = new Polyline({
    points: [
      new Vector3(-2.0, 0.0, 1.6),
      new Vector3(-1.4, 0.0, 2.4),
      new Vector3(-0.7, 0.0, 1.8),
      new Vector3(-0.2, 0.0, 2.6),
    ],
    color: 0x0284c7,
  });

  const arc = new Arc({
    center: new Vector3(1.0, 0.0, -1.0),
    radius: 1.0,
    startAngle: 0.0,
    endAngle: Math.PI * 1.5,
    segments: 32,
    color: 0xb91c1c,
  });

  const rectangle = new Rectangle({
    center: new Vector3(2.0, 0.0, 1.8),
    width: 1.5,
    breadth: 0.9,
    color: 0x1d4ed8,
  });

  const curve = new Curve({
    controlPoints: [
      new Vector3(-0.8, 0.0, -2.2),
      new Vector3(0.0, 0.0, -2.4),
      new Vector3(0.9, 0.0, -1.8),
      new Vector3(1.6, 0.0, -2.0),
    ],
    color: 0x7c3aed,
  });

  scene.add(line);
  scene.add(polyline);
  scene.add(arc);
  scene.add(rectangle);
  scene.add(curve);

  return { line, polyline, arc, rectangle, curve };
}
