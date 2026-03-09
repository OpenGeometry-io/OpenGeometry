import { Rectangle, Vector3 } from "@og-three";
import { defineExample } from "../../shared/example-contract";
import { mountControls, replaceSceneObject } from "../../shared/runtime";

export default defineExample({
  slug: "primitives/rectangle",
  category: "primitives",
  title: "Rectangle",
  description: "Parametric rectangular primitive for base profiles.",
  statusLabel: "ready",
  chips: ["Control: Width", "Control: Breadth"],
  footerText: "Control: Width, Breadth",
  build: ({ scene }) => {
    let current: Rectangle | null = null;

    mountControls(
      "Rectangle Parameters",
      [
        { type: "number", key: "width", label: "Width", min: 0.2, max: 5, step: 0.05, value: 1.8 },
        { type: "number", key: "breadth", label: "Breadth", min: 0.2, max: 5, step: 0.05, value: 1.1 },
        { type: "number", key: "centerX", label: "Center X", min: -3, max: 3, step: 0.1, value: 0 },
        { type: "number", key: "centerZ", label: "Center Z", min: -3, max: 3, step: 0.1, value: 0 },
      ],
      (state) => {
        const rectangle = new Rectangle({
          center: new Vector3(state.centerX as number, 0, state.centerZ as number),
          width: state.width as number,
          breadth: state.breadth as number,
          color: 0x1f2937,
        });

        current = replaceSceneObject(scene, current, rectangle);
      }
    );
  },
});
