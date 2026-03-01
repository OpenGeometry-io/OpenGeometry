import { Cuboid, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

bootstrapExample({
  title: "Shape: Cuboid",
  description: "Interactive cuboid BREP with outline controls.",
  build: ({ scene }) => {
    let current: Cuboid | null = null;

    mountControls(
      "Cuboid Parameters",
      [
        { type: "number", key: "width", label: "Width", min: 0.2, max: 4, step: 0.05, value: 1.8 },
        { type: "number", key: "height", label: "Height", min: 0.2, max: 4, step: 0.05, value: 1.6 },
        { type: "number", key: "depth", label: "Depth", min: 0.2, max: 4, step: 0.05, value: 1.2 },
        { type: "boolean", key: "outline", label: "Outline", value: true },
      ],
      (state) => {
        const cuboid = new Cuboid({
          center: new Vector3(0, (state.height as number) * 0.5, 0),
          width: state.width as number,
          height: state.height as number,
          depth: state.depth as number,
          color: 0x10b981,
        });
        cuboid.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, cuboid);
      }
    );
  },
});
