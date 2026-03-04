import * as THREE from "three";
import { Vector3 } from "../../../../opengeometry/pkg/opengeometry";
import { BooleanShape, Cuboid, Sphere } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

const operations = ["union", "intersection", "difference"] as const;

void bootstrapExample({
  title: "Boolean (Union / Intersection / Difference)",
  description:
    "Robust BSP-style constructive solid geometry over any OpenGeometry triangulated shape.",
  build: ({ scene }) => {
    let result: THREE.Mesh | null = null;

    const update = (state: Record<string, number | boolean>) => {
      const left = new Cuboid({
        center: new Vector3(-0.4, 0.9, 0),
        width: state.leftWidth as number,
        height: 1.8,
        depth: 1.5,
        color: 0x10b981,
      });
      left.outline = true;

      const right = new Sphere({
        center: new Vector3(0.5, 0.9, 0),
        radius: state.rightRadius as number,
        widthSegments: 30,
        heightSegments: 20,
        color: 0xf97316,
      });
      right.outline = true;

      const operation = operations[Math.round(state.operation as number)] ?? "union";
      const boolean = new BooleanShape(left, right, operation, {
        epsilon: state.epsilon as number,
        snap: state.snap as number,
      });

      boolean.material = new THREE.MeshStandardMaterial({
        color: 0x2563eb,
        transparent: true,
        opacity: 0.72,
      });

      scene.add(left);
      scene.add(right);
      result = replaceSceneObject(scene, result, boolean);

      left.removeFromParent();
      right.removeFromParent();
    };

    mountControls(
      "Boolean Controls",
      [
        { type: "number", key: "operation", label: "Operation (0=Union,1=Intersect,2=Diff)", min: 0, max: 2, step: 1, value: 0 },
        { type: "number", key: "leftWidth", label: "Left Cuboid Width", min: 0.8, max: 2.2, step: 0.1, value: 1.4 },
        { type: "number", key: "rightRadius", label: "Right Sphere Radius", min: 0.5, max: 1.2, step: 0.05, value: 0.85 },
        { type: "number", key: "epsilon", label: "Plane Epsilon", min: 0.000001, max: 0.005, step: 0.000001, value: 0.00001 },
        { type: "number", key: "snap", label: "Snap Grid", min: 0.000001, max: 0.005, step: 0.000001, value: 0.00001 },
      ],
      update
    );

    update({
      operation: 0,
      leftWidth: 1.4,
      rightRadius: 0.85,
      epsilon: 0.00001,
      snap: 0.00001,
    });
  },
});
