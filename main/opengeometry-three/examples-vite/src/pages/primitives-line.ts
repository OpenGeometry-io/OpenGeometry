import { Line, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

bootstrapExample({
  title: "Primitive: Line",
  description: "Interactive line primitive with live parameter controls.",
  build: ({ scene }) => {
    let current: Line | null = null;

    mountControls(
      "Line Parameters",
      [
        { type: "number", key: "length", label: "Length", min: 0.4, max: 8, step: 0.1, value: 4 },
        { type: "number", key: "angleDeg", label: "Angle (deg)", min: 0, max: 360, step: 1, value: 35 },
        { type: "number", key: "centerX", label: "Center X", min: -3, max: 3, step: 0.1, value: 0 },
        { type: "number", key: "centerZ", label: "Center Z", min: -3, max: 3, step: 0.1, value: 0 },
      ],
      (state) => {
        const length = state.length as number;
        const angle = ((state.angleDeg as number) * Math.PI) / 180;
        const cx = state.centerX as number;
        const cz = state.centerZ as number;

        const halfDx = Math.cos(angle) * (length * 0.5);
        const halfDz = Math.sin(angle) * (length * 0.5);

        const line = new Line({
          start: new Vector3(cx - halfDx, 0, cz - halfDz),
          end: new Vector3(cx + halfDx, 0, cz + halfDz),
          color: 0x111827,
        });

        current = replaceSceneObject(scene, current, line);
      }
    );
  },
});
