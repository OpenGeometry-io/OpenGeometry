import { Cylinder, Vector3 } from "@og-three";
import { defineExample } from "../../shared/example-contract";
import { mountControls, replaceSceneObject } from "../../shared/runtime";

export default defineExample({
  slug: "shapes/cylinder",
  category: "shapes",
  title: "Cylinder",
  description: "Cylindrical volume for ducts, pipes and mechanical shafts.",
  statusLabel: "ready",
  chips: ["Control: R", "Control: H", "Control: Seg"],
  footerText: "Control: R, H, Seg",
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
        { type: "boolean", key: "fatOutlines", label: "Fat Outlines", value: false },
        { type: "number", key: "outlineWidth", label: "Outline Width", min: 1, max: 12, step: 0.5, value: 4 },
      ],
      (state) => {
        const cylinder = new Cylinder({
          center: new Vector3(0, (state.height as number) * 0.5, 0),
          radius: state.radius as number,
          height: state.height as number,
          segments: Math.floor(state.segments as number),
          angle: ((state.angleDeg as number) * Math.PI) / 180,
          color: 0xf59e0b,
          fatOutlines: state.fatOutlines as boolean,
          outlineWidth: state.outlineWidth as number,
        });
        cylinder.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, cylinder);
      }
    );
  },
});
