import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Cuboid } from "../shapes/cuboid";
import { Cylinder } from "../shapes/cylinder";
import { Opening } from "../shapes/opening";
import { Polygon } from "../shapes/polygon";
import { Sphere } from "../shapes/sphere";
import { Sweep } from "../shapes/sweep";
import { Wedge } from "../shapes/wedge";

/**
 * Adds a basic shapes showcase to the provided scene.
 */
export function createShapesExample(scene: THREE.Scene) {
  const polygon = new Polygon({
    vertices: [
      new Vector3(-3.0, 0.0, -0.5),
      new Vector3(-2.2, 0.0, 0.5),
      new Vector3(-1.2, 0.0, 0.2),
      new Vector3(-1.5, 0.0, -0.8),
    ],
    color: 0x2563eb,
  });

  const cuboid = new Cuboid({
    center: new Vector3(0.0, 0.8, -1.2),
    width: 1.2,
    height: 1.6,
    depth: 1.0,
    color: 0x10b981,
  });
  cuboid.outline = true;

  const cylinder = new Cylinder({
    center: new Vector3(1.8, 0.8, -0.8),
    radius: 0.6,
    height: 1.6,
    segments: 28,
    angle: Math.PI * 2,
    color: 0xf97316,
  });
  cylinder.outline = true;

  const wedge = new Wedge({
    center: new Vector3(3.4, 0.8, -1.0),
    width: 1.2,
    height: 1.4,
    depth: 1.0,
    color: 0x7c3aed,
  });
  wedge.outline = true;

  const sphere = new Sphere({
    center: new Vector3(0.6, 1.0, 1.2),
    radius: 0.9,
    widthSegments: 28,
    heightSegments: 18,
    color: 0x0ea5e9,
  });
  sphere.outline = true;

  const opening = new Opening({
    center: new Vector3(2.4, 0.9, 1.2),
    width: 1.0,
    height: 1.8,
    depth: 0.3,
    color: 0x9ca3af,
  });
  opening.outline = true;

  const sweep = new Sweep({
    path: [
      new Vector3(-2.8, 0.0, 2.4),
      new Vector3(-2.2, 0.6, 2.8),
      new Vector3(-1.4, 1.2, 2.4),
      new Vector3(-0.8, 1.7, 1.8),
    ],
    profile: [
      new Vector3(-0.2, 0.0, -0.2),
      new Vector3(0.2, 0.0, -0.2),
      new Vector3(0.2, 0.0, 0.2),
      new Vector3(-0.2, 0.0, 0.2),
    ],
    color: 0x14b8a6,
    capStart: true,
    capEnd: true,
  });
  sweep.outline = true;

  scene.add(polygon);
  scene.add(cuboid);
  scene.add(cylinder);
  scene.add(wedge);
  scene.add(sphere);
  scene.add(opening);
  scene.add(sweep);

  return { polygon, cuboid, cylinder, wedge, sphere, opening, sweep };
}
