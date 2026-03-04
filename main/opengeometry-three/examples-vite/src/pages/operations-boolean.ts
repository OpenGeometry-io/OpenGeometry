import { Cuboid, Cylinder, Sphere, Vector3, booleanOperation, type BooleanOperationType } from "@og-three";
import * as THREE from "three";
import { bootstrapExample, mountControls, replaceSceneObject } from "../shared/runtime";

function resolveOperation(mode: number): BooleanOperationType {
  if (mode < 0.5) {
    return "union";
  }
  if (mode < 1.5) {
    return "subtract";
  }
  return "intersect";
}

bootstrapExample({
  title: "Operation: Boolean",
  description: "Voxel-robust boolean operation between any two OpenGeometry shapes.",
  build: ({ scene }) => {
    let resultMesh: THREE.Mesh | null = null;

    mountControls(
      "Boolean Parameters",
      [
        { type: "number", key: "operation", label: "Operation (0:union,1:subtract,2:intersect)", min: 0, max: 2, step: 1, value: 0 },
        { type: "number", key: "resolution", label: "Grid Resolution", min: 8, max: 40, step: 1, value: 26 },
        { type: "number", key: "offset", label: "Offset", min: -1.2, max: 1.2, step: 0.05, value: 0.35 },
        { type: "boolean", key: "positiveY", label: "Clamp to Positive Y", value: false },
      ],
      (state) => {
        const left = new Cuboid({
          center: new Vector3(-0.2, 0, 0),
          width: 2.2,
          height: 1.8,
          depth: 2.0,
          color: 0x9ca3af,
        });

        const right = new Cylinder({
          center: new Vector3(state.offset as number, 0, 0.15),
          radius: 0.9,
          height: 2.2,
          segments: 48,
          color: 0x6b7280,
        });

        const cap = new Sphere({
          center: new Vector3((state.offset as number) * 0.5, 0.35, 0),
          radius: 0.85,
          widthSegments: 28,
          heightSegments: 18,
          color: 0x4b5563,
        });

        const stagedRight = booleanOperation(right, cap, {
          operation: "union",
          gridResolution: Math.max(8, (state.resolution as number) - 6),
          color: 0x6b7280,
          opacity: 0.2,
        });

        const result = booleanOperation(left, stagedRight, {
          operation: resolveOperation(state.operation as number),
          gridResolution: state.resolution as number,
          constrainResultToPositiveY: state.positiveY as boolean,
          color: 0x2563eb,
          opacity: 0.72,
        });

        resultMesh = replaceSceneObject(scene, resultMesh, result);
      }
    );
  },
});
