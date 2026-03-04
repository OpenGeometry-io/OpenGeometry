import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Cuboid } from "../shapes/cuboid";
import { Cylinder } from "../shapes/cylinder";
import { booleanOperation } from "../operations";

export function createBooleanExample(scene: THREE.Scene) {
  const left = new Cuboid({
    center: new Vector3(0, 0, 0),
    width: 2,
    height: 2,
    depth: 2,
    color: 0x6b7280,
  });

  const right = new Cylinder({
    center: new Vector3(0.55, 0, 0),
    radius: 0.95,
    height: 2.2,
    segments: 42,
    color: 0x9ca3af,
  });

  const result = booleanOperation(left, right, {
    operation: "subtract",
    gridResolution: 30,
    color: 0x2563eb,
  });

  scene.add(result);

  return { left, right, result };
}
