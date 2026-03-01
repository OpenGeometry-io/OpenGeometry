import * as THREE from "three";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Polyline } from "../primitives/polyline";
import { Rectangle } from "../primitives/rectangle";
import { Sweep } from "../shapes/sweep";

type KernelVertex = {
  position: {
    x: number;
    y: number;
    z: number;
  };
};

type KernelBrep = {
  vertices: KernelVertex[];
};

function toVector3List(vertices: KernelVertex[]): Vector3[] {
  return vertices.map((vertex) => {
    const { x, y, z } = vertex.position;
    return new Vector3(x, y, z);
  });
}

export function createSweepExample(scene: THREE.Scene) {
  const pathPrimitive = new Polyline({
    points: [
      new Vector3(-2.0, 0.0, -1.0),
      new Vector3(-1.2, 0.5, 0.2),
      new Vector3(0.0, 1.1, 0.9),
      new Vector3(1.1, 1.7, 0.3),
      new Vector3(2.0, 2.2, -0.8),
    ],
    color: 0x444444,
  });

  const profilePrimitive = new Rectangle({
    center: new Vector3(0, 0, 0),
    width: 0.7,
    breadth: 0.4,
    color: 0x1f77b4,
  });

  // const profilePrimitive = new Arc();

  const profileBrep = profilePrimitive.getBrep() as KernelBrep;
  const profilePoints = toVector3List(profileBrep.vertices);

  const sweep = new Sweep({
    path: pathPrimitive.options.points.map((point) => point.clone()),
    profile: profilePoints,
    color: 0x2a9d8f,
    capStart: true,
    capEnd: false,
  });

  scene.add(pathPrimitive);
  scene.add(profilePrimitive);
  scene.add(sweep);

  return {
    pathPrimitive,
    profilePrimitive,
    sweep,
  };
}
