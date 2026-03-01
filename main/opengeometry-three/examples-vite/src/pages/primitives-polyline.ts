import { Polyline, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function buildPolyline(amplitude: number, length: number, closed: boolean): Vector3[] {
  const points = [
    new Vector3(-length, 0, -amplitude),
    new Vector3(-length * 0.35, 0, amplitude),
    new Vector3(length * 0.2, 0, -amplitude * 0.65),
    new Vector3(length, 0, amplitude * 0.8),
  ];

  if (closed) {
    points.push(points[0].clone());
  }

  return points;
}

bootstrapExample({
  title: "Primitive: Polyline",
  description: "Interactive open/closed polyline configurations.",
  build: ({ scene }) => {
    let current: Polyline | null = null;

    mountControls(
      "Polyline Parameters",
      [
        { type: "number", key: "amplitude", label: "Amplitude", min: 0.2, max: 2.5, step: 0.05, value: 1.1 },
        { type: "number", key: "length", label: "Length", min: 0.4, max: 3, step: 0.05, value: 2.2 },
        { type: "boolean", key: "closed", label: "Closed", value: false },
      ],
      (state) => {
        const polyline = new Polyline({
          points: buildPolyline(state.amplitude as number, state.length as number, state.closed as boolean),
          color: 0x1d4ed8,
        });

        current = replaceSceneObject(scene, current, polyline);
      }
    );
  },
});
