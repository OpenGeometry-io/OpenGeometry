import { Sweep, Vector3 } from "@og-three";
import {
  bootstrapExample,
  mountControls,
  replaceSceneObject,
} from "../shared/runtime";

function buildPath(height: number, spread: number): Vector3[] {
  return [
    new Vector3(-2.0, 0.0, -1.2),
    new Vector3(-1.0, height * 0.25, -0.2 * spread),
    new Vector3(0.2, height * 0.55, 0.7 * spread),
    new Vector3(1.3, height * 0.8, 0.2 * spread),
    new Vector3(2.1, height, -0.9 * spread),
  ];
}

function buildProfile(width: number, depth: number): Vector3[] {
  return [
    new Vector3(-width * 0.5, 0, -depth * 0.5),
    new Vector3(width * 0.5, 0, -depth * 0.5),
    new Vector3(width * 0.5, 0, depth * 0.5),
    new Vector3(-width * 0.5, 0, depth * 0.5),
  ];
}

bootstrapExample({
  title: "Shape: Sweep",
  description: "Interactive profile sweep along a 3D path.",
  build: ({ scene }) => {
    let current: Sweep | null = null;

    mountControls(
      "Sweep Parameters",
      [
        { type: "number", key: "height", label: "Path Height", min: 0.4, max: 4, step: 0.05, value: 2.3 },
        { type: "number", key: "spread", label: "Path Spread", min: 0.4, max: 2, step: 0.05, value: 1.0 },
        { type: "number", key: "profileWidth", label: "Profile Width", min: 0.1, max: 1.5, step: 0.05, value: 0.5 },
        { type: "number", key: "profileDepth", label: "Profile Depth", min: 0.1, max: 1.5, step: 0.05, value: 0.4 },
        { type: "boolean", key: "capStart", label: "Cap Start", value: true },
        { type: "boolean", key: "capEnd", label: "Cap End", value: true },
        { type: "boolean", key: "outline", label: "Outline", value: true },
      ],
      (state) => {
        const sweep = new Sweep({
          path: buildPath(state.height as number, state.spread as number),
          profile: buildProfile(state.profileWidth as number, state.profileDepth as number),
          color: 0x0ea5e9,
          capStart: state.capStart as boolean,
          capEnd: state.capEnd as boolean,
        });
        sweep.outline = state.outline as boolean;

        current = replaceSceneObject(scene, current, sweep);
      }
    );
  },
});
