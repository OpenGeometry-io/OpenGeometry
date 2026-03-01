import { Vector3, Wedge } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

bootstrapExample({
  title: "Shape: Wedge",
  description: "Interactive wedge primitive with size controls.",
  build: ({ scene }) => {
    let current: Wedge | null = null;

    mountControls(
      "Wedge Parameters",
      [
        { type: "number", key: "width", label: "Width", min: 0.2, max: 4, step: 0.05, value: 2.0 },
        { type: "number", key: "height", label: "Height", min: 0.2, max: 4, step: 0.05, value: 1.8 },
        { type: "number", key: "depth", label: "Depth", min: 0.2, max: 4, step: 0.05, value: 1.4 },
        { type: "boolean", key: "outline", label: "Outline", value: true },
      ],
      (state) => {
        const wedge = new Wedge({
          center: new Vector3(0, (state.height as number) * 0.5, 0),
          width: state.width as number,
          height: state.height as number,
          depth: state.depth as number,
          color: 0x7c3aed,
        });
        wedge.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, wedge);
      }
    );
  },
});
