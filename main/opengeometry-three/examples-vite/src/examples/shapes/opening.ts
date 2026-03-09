import { Opening, Vector3 } from "@og-three";
import { defineExample } from "../../shared/example-contract";
import { mountControls, replaceSceneObject } from "../../shared/runtime";

export default defineExample({
  slug: "shapes/opening",
  category: "shapes",
  title: "Opening",
  description: "Opening helper volume for void and penetration previews.",
  statusLabel: "ready",
  chips: ["Control: W/H/D"],
  footerText: "Control: W/H/D",
  build: ({ scene }) => {
    let current: Opening | null = null;

    mountControls(
      "Opening Parameters",
      [
        { type: "number", key: "width", label: "Width", min: 0.2, max: 4, step: 0.05, value: 1.3 },
        { type: "number", key: "height", label: "Height", min: 0.2, max: 4, step: 0.05, value: 2.0 },
        { type: "number", key: "depth", label: "Depth", min: 0.1, max: 2, step: 0.05, value: 0.35 },
        { type: "boolean", key: "outline", label: "Outline", value: true },
        { type: "boolean", key: "fatOutlines", label: "Fat Outlines", value: false },
        { type: "number", key: "outlineWidth", label: "Outline Width", min: 1, max: 12, step: 0.5, value: 4 },
      ],
      (state) => {
        const opening = new Opening({
          center: new Vector3(0, (state.height as number) * 0.5, 0),
          width: state.width as number,
          height: state.height as number,
          depth: state.depth as number,
          color: 0x94a3b8,
          fatOutlines: state.fatOutlines as boolean,
          outlineWidth: state.outlineWidth as number,
        });
        opening.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, opening);
      }
    );
  },
});
