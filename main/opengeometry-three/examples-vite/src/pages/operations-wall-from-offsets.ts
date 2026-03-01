import { Polygon, Polyline, Vector3 } from "@og-three";
import * as THREE from "three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function buildCenterline(curveBias: number): Vector3[] {
  return [
    new Vector3(-2.6, 0, -1.9),
    new Vector3(-1.2, 0, -1.0),
    new Vector3(-0.2, 0, 0.1),
    new Vector3(0.7 + curveBias, 0, 0.2),
    new Vector3(0.1, 0, 1.0),
    new Vector3(2.6, 0, 2.0),
  ];
}

function buildWallOutline(left: Vector3[], right: Vector3[]): Vector3[] {
  if (left.length === 0 || right.length === 0) {
    return [];
  }

  return [...left.map((p) => p.clone()), ...right.map((p) => p.clone()).reverse()];
}

bootstrapExample({
  title: "Operation: Wall from Offsets",
  description: "Interactive wall polygon generated from +/- polyline offsets.",
  build: ({ scene }) => {
    let current: THREE.Group | null = null;

    mountControls(
      "Wall Parameters",
      [
        { type: "number", key: "thickness", label: "Wall Thickness", min: 0.1, max: 2, step: 0.05, value: 0.45 },
        { type: "number", key: "acute", label: "Acute Threshold", min: 1, max: 179, step: 1, value: 90 },
        { type: "number", key: "curveBias", label: "Curve Bias", min: -0.8, max: 0.8, step: 0.05, value: 0.0 },
        { type: "boolean", key: "bevel", label: "Bevel", value: true },
      ],
      (state) => {
        const centerline = new Polyline({
          points: buildCenterline(state.curveBias as number),
          color: 0x1f2937,
        });

        const half = (state.thickness as number) * 0.5;
        const leftOffset = centerline.getOffset(half, state.acute as number, state.bevel as boolean);
        const rightOffset = centerline.getOffset(-half, state.acute as number, state.bevel as boolean);

        const leftPolyline = new Polyline({ points: leftOffset.points, color: 0x22c55e });
        const rightPolyline = new Polyline({ points: rightOffset.points, color: 0xf97316 });

        const outline = buildWallOutline(leftOffset.points, rightOffset.points);
        if (outline.length < 3) {
          return;
        }

        const polygon = new Polygon({ vertices: outline, color: 0x3b82f6 });
        polygon.position.y = 0.01;
        polygon.outline = true;

        const group = new THREE.Group();
        group.add(centerline);
        group.add(leftPolyline);
        group.add(rightPolyline);
        group.add(polygon);

        current = replaceSceneObject(scene, current, group);
      }
    );
  },
});
