import { Cuboid, Vector3 } from "@og-three";
import { defineExample } from "../../shared/example-contract";
import { mountControls, replaceSceneObject } from "../../shared/runtime";

export default defineExample({
  slug: "shapes/cuboid",
  category: "shapes",
  title: "Cuboid",
  description: "Rectangular solid for rooms, equipment blocks and massing.",
  statusLabel: "ready",
  chips: ["Width", "Height", "Depth"],
  footerText: "Width, Height, Depth",
  build: ({ scene }) => {
    let current: Cuboid | null = null;

    mountControls(
      "Cuboid Parameters",
      [
        { type: "number", key: "width", label: "Width", min: 0.2, max: 4, step: 0.05, value: 1.8 },
        { type: "number", key: "height", label: "Height", min: 0.2, max: 4, step: 0.05, value: 1.6 },
        { type: "number", key: "depth", label: "Depth", min: 0.2, max: 4, step: 0.05, value: 1.2 },
        { type: "boolean", key: "outline", label: "Outline", value: true },
        { type: "boolean", key: "fatOutlines", label: "Fat Outlines", value: false },
        { type: "number", key: "outlineWidth", label: "Outline Width", min: 1, max: 12, step: 0.5, value: 4 },
      ],
      (state) => {
        const cuboid = new Cuboid({
          center: new Vector3(0, (state.height as number) * 0.5, 0),
          width: state.width as number,
          height: state.height as number,
          depth: state.depth as number,
          color: 0x10b981,
          fatOutlines: state.fatOutlines as boolean,
          outlineWidth: state.outlineWidth as number,
        });
        cuboid.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, cuboid);
      }
    );
  },
});
