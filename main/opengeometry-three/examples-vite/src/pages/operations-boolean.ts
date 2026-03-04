import { BooleanMesh, Cuboid, Vector3 } from "@og-three";
import * as THREE from "three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

bootstrapExample({
  title: "Operation: Boolean",
  description:
    "Voxel-constraint boolean operation between two cuboids (union / intersection / difference).",
  build: ({ scene }) => {
    let current: THREE.Group | null = null;

    mountControls(
      "Boolean Parameters",
      [
        {
          type: "select",
          key: "operation",
          label: "Operation",
          value: "union",
          options: [
            { label: "Union", value: "union" },
            { label: "Intersection", value: "intersection" },
            { label: "Difference (A - B)", value: "difference" },
          ],
        },
        { type: "number", key: "offsetX", label: "B Offset X", min: -1.25, max: 1.25, step: 0.05, value: 0.45 },
        { type: "number", key: "voxelSize", label: "Voxel Size", min: 0.08, max: 0.4, step: 0.02, value: 0.15 },
      ],
      (state) => {
        const a = new Cuboid({
          center: new Vector3(-0.2, 0, 0),
          width: 1.4,
          height: 1.2,
          depth: 1.1,
          color: 0x1d4ed8,
        });

        const b = new Cuboid({
          center: new Vector3(state.offsetX as number, 0, 0),
          width: 1.2,
          height: 1.2,
          depth: 1.3,
          color: 0xdc2626,
        });

        const result = new BooleanMesh();
        result.compute(a, b, state.operation as "union" | "intersection" | "difference", {
          voxelSize: state.voxelSize as number,
          color: 0x16a34a,
          opacity: 0.8,
        });

        const group = new THREE.Group();
        group.add(result);

        current = replaceSceneObject(scene, current, group);
      }
    );
  },
});
