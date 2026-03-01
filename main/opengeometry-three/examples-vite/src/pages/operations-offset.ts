import { Polyline, Vector3 } from "@og-three";
import * as THREE from "three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function createBasePolyline(turn: number): Vector3[] {
  return [
    new Vector3(-2.4, 0, -1.8),
    new Vector3(-0.7, 0, -0.6),
    new Vector3(-0.2 + turn, 0, 0.7),
    new Vector3(1.9, 0, 1.6),
  ];
}

bootstrapExample({
  title: "Operation: Offset",
  description: "Interactive offset operation on a polyline primitive.",
  build: ({ scene }) => {
    let current: THREE.Group | null = null;

    mountControls(
      "Offset Parameters",
      [
        { type: "number", key: "offset", label: "Offset", min: -1.2, max: 1.2, step: 0.05, value: 0.45 },
        { type: "number", key: "acute", label: "Acute Threshold", min: 1, max: 179, step: 1, value: 90 },
        { type: "number", key: "turn", label: "Path Turn", min: -0.8, max: 0.8, step: 0.05, value: 0.0 },
        { type: "boolean", key: "bevel", label: "Bevel", value: true },
      ],
      (state) => {
        const base = new Polyline({
          points: createBasePolyline(state.turn as number),
          color: 0x1f2937,
        });

        const offsetData = base.getOffset(
          state.offset as number,
          state.acute as number,
          state.bevel as boolean
        );

        const offset = new Polyline({
          points: offsetData.points,
          color: 0xdc2626,
        });

        const group = new THREE.Group();
        group.add(base);
        group.add(offset);

        current = replaceSceneObject(scene, current, group);
      }
    );
  },
});
