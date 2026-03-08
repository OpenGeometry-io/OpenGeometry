import { Sweep, Vector3 } from "../../../../../dist/index.js";
import {
  bootstrapSweepDemo,
  createPathGuide,
  createProfileGuide,
} from "../shared/demo-runtime.js";
import {
  applySweepAppearance,
  baseSweepControls,
  createSquareProfile,
} from "./common.js";

function buildOpenPath(legA, legB, zDrift) {
  return [new Vector3(0, 0, 0), new Vector3(legA, 0, 0), new Vector3(legA, legB, zDrift)];
}

const controls = [
  { type: "number", key: "legA", label: "First Leg", min: 1, max: 14, step: 0.1, value: 5 },
  { type: "number", key: "legB", label: "Second Leg", min: 1, max: 14, step: 0.1, value: 5 },
  { type: "number", key: "zDrift", label: "3D Drift", min: -6, max: 6, step: 0.1, value: 0 },
  { type: "number", key: "profileWidth", label: "Profile Width", min: 0.1, max: 3, step: 0.05, value: 1 },
  { type: "number", key: "profileDepth", label: "Profile Depth", min: 0.1, max: 3, step: 0.05, value: 1 },
  ...baseSweepControls(0xe74c3c, false, false),
];

bootstrapSweepDemo({
  title: "Sweep Usage: Open-Ended Sweep",
  description: "L-shaped tube setup with open caps by default.",
  panelTitle: "Open-Ended Sweep Parameters",
  controls,
  cameraPosition: [9, 7, 10],
  controlsTarget: [2.4, 2.2, 0],
  createSceneObject: ({ state, THREE }) => {
    const path = buildOpenPath(state.legA, state.legB, state.zDrift);
    const profile = createSquareProfile(state.profileWidth, state.profileDepth);

    const openTube = new Sweep({
      path,
      profile,
      color: state.color,
      capStart: state.capStart,
      capEnd: state.capEnd,
      outlineWidth: state.outlineWidth,
      fatOutlines: state.fatOutlines,
    });
    applySweepAppearance(openTube, state);

    const group = new THREE.Group();
    const pathGuide = createPathGuide(path, 0x475569);
    pathGuide.position.y += 0.01;

    const profileGuide = createProfileGuide(profile, new THREE.Vector3(-3.8, 0, -3.4), 0x0f172a);

    group.add(openTube);
    group.add(pathGuide);
    group.add(profileGuide);
    return group;
  },
});
