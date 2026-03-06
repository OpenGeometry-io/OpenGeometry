import { createShapeOutlineDropdownExample } from "@og-three";
import { bootstrapExample } from "../shared/runtime";

bootstrapExample({
  title: "Shape: CAD Outline (HLR)",
  description:
    "Dropdown-driven shape preview with camera-projected hidden-line removal outlines.",
  build: ({ scene, camera, renderer, controls }) => {
    createShapeOutlineDropdownExample({
      scene,
      camera,
      renderer,
      controls,
      initialShape: "cuboid",
      initialRefreshMode: "live",
      initialOutlineEnabled: true,
    });
  },
});
