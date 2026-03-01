import { Curve, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function buildControlPoints(span: number, sag: number, lift: number): Vector3[] {
  return [
    new Vector3(-span, 0, -0.6),
    new Vector3(-span * 0.4, lift, -sag),
    new Vector3(span * 0.2, lift * 0.3, sag),
    new Vector3(span, 0, 0.8),
  ];
}

bootstrapExample({
  title: "Primitive: Curve",
  description: "Interactive curve primitive defined by control points.",
  build: ({ scene }) => {
    let current: Curve | null = null;

    mountControls(
      "Curve Parameters",
      [
        { type: "number", key: "span", label: "Span", min: 0.6, max: 3, step: 0.05, value: 2.4 },
        { type: "number", key: "sag", label: "Sag", min: 0.1, max: 2, step: 0.05, value: 1.1 },
        { type: "number", key: "lift", label: "Lift", min: 0, max: 2, step: 0.05, value: 0.7 },
      ],
      (state) => {
        const curve = new Curve({
          controlPoints: buildControlPoints(
            state.span as number,
            state.sag as number,
            state.lift as number
          ),
          color: 0x0f766e,
        });

        current = replaceSceneObject(scene, current, curve);
      }
    );
  },
});
