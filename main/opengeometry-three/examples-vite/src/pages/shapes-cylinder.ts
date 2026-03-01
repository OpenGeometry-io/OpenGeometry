import { Cylinder, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

bootstrapExample({
  title: "Shape: Cylinder",
  description: "Interactive cylinder with segment and angle controls.",
  build: ({ scene }) => {
    let current: Cylinder | null = null;

    mountControls(
      "Cylinder Parameters",
      [
        { type: "number", key: "radius", label: "Radius", min: 0.2, max: 3, step: 0.05, value: 0.8 },
        { type: "number", key: "height", label: "Height", min: 0.2, max: 4, step: 0.05, value: 1.8 },
        { type: "number", key: "segments", label: "Segments", min: 6, max: 96, step: 1, value: 36 },
        { type: "number", key: "angleDeg", label: "Angle (deg)", min: 20, max: 360, step: 1, value: 360 },
        { type: "boolean", key: "outline", label: "Outline", value: true },
      ],
      (state) => {
        const cylinder = new Cylinder({
          center: new Vector3(0, (state.height as number) * 0.5, 0),
          radius: state.radius as number,
          height: state.height as number,
          segments: Math.floor(state.segments as number),
          angle: ((state.angleDeg as number) * Math.PI) / 180,
          color: 0xf59e0b,
        });
        cylinder.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, cylinder);
      }
    );
  },
});
