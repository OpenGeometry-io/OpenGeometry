import { Arc, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

bootstrapExample({
  title: "Primitive: Arc",
  description: "Interactive arc with radius, angle span, and segment controls.",
  build: ({ scene }) => {
    let current: Arc | null = null;

    mountControls(
      "Arc Parameters",
      [
        { type: "number", key: "radius", label: "Radius", min: 0.2, max: 4, step: 0.05, value: 1.2 },
        { type: "number", key: "startDeg", label: "Start (deg)", min: 0, max: 360, step: 1, value: 0 },
        { type: "number", key: "endDeg", label: "End (deg)", min: 1, max: 360, step: 1, value: 300 },
        { type: "number", key: "segments", label: "Segments", min: 4, max: 128, step: 1, value: 48 },
        { type: "number", key: "centerX", label: "Center X", min: -3, max: 3, step: 0.1, value: 0 },
        { type: "number", key: "centerZ", label: "Center Z", min: -3, max: 3, step: 0.1, value: 0 },
      ],
      (state) => {
        const start = ((state.startDeg as number) * Math.PI) / 180;
        const end = ((state.endDeg as number) * Math.PI) / 180;

        const arc = new Arc({
          center: new Vector3(state.centerX as number, 0, state.centerZ as number),
          radius: state.radius as number,
          startAngle: Math.min(start, end),
          endAngle: Math.max(start, end),
          segments: Math.floor(state.segments as number),
          color: 0xdc2626,
        });

        current = replaceSceneObject(scene, current, arc);
      }
    );
  },
});
