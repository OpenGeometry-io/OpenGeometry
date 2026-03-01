import { Polyline, Rectangle, Sweep, Vector3 } from "@og-three";
import * as THREE from "three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

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

function buildPath(height: number): Vector3[] {
  return [
    new Vector3(-2.0, 0.0, -1.0),
    new Vector3(-1.2, height * 0.2, 0.2),
    new Vector3(0.0, height * 0.5, 0.9),
    new Vector3(1.1, height * 0.78, 0.3),
    new Vector3(2.0, height, -0.8),
  ];
}

bootstrapExample({
  title: "Operation: Sweep Path + Profile",
  description: "Interactive sweep generated from path and profile primitives.",
  build: ({ scene }) => {
    let current: THREE.Group | null = null;

    mountControls(
      "Sweep Operation Parameters",
      [
        { type: "number", key: "pathHeight", label: "Path Height", min: 0.4, max: 4, step: 0.05, value: 2.2 },
        { type: "number", key: "profileWidth", label: "Profile Width", min: 0.1, max: 1.5, step: 0.05, value: 0.7 },
        { type: "number", key: "profileDepth", label: "Profile Depth", min: 0.1, max: 1.5, step: 0.05, value: 0.4 },
        { type: "boolean", key: "capStart", label: "Cap Start", value: true },
        { type: "boolean", key: "capEnd", label: "Cap End", value: false },
      ],
      (state) => {
        const pathPrimitive = new Polyline({
          points: buildPath(state.pathHeight as number),
          color: 0x444444,
        });

        const profilePrimitive = new Rectangle({
          center: new Vector3(0, 0, 0),
          width: state.profileWidth as number,
          breadth: state.profileDepth as number,
          color: 0x1f77b4,
        });

        const profileBrep = profilePrimitive.getBrep() as KernelBrep;
        const profilePoints = toVector3List(profileBrep.vertices);

        const sweep = new Sweep({
          path: pathPrimitive.options.points.map((point) => point.clone()),
          profile: profilePoints,
          color: 0x2a9d8f,
          capStart: state.capStart as boolean,
          capEnd: state.capEnd as boolean,
        });
        sweep.outline = true;

        pathPrimitive.position.y += 0.01;
        profilePrimitive.position.set(-3.0, 0.0, -2.2);

        const group = new THREE.Group();
        group.add(pathPrimitive);
        group.add(profilePrimitive);
        group.add(sweep);

        current = replaceSceneObject(scene, current, group);
      }
    );
  },
});
