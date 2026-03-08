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

function buildPath(height, baseX, baseZ) {
  return [new Vector3(baseX, 0, baseZ), new Vector3(baseX, height, baseZ)];
}

const controls = [
  { type: "number", key: "height", label: "Pipe Height", min: 1, max: 20, step: 0.25, value: 10 },
  { type: "number", key: "profileWidth", label: "Profile Width", min: 0.1, max: 3, step: 0.05, value: 1 },
  { type: "number", key: "profileDepth", label: "Profile Depth", min: 0.1, max: 3, step: 0.05, value: 1 },
  { type: "number", key: "baseX", label: "Base X", min: -6, max: 6, step: 0.1, value: 0 },
  { type: "number", key: "baseZ", label: "Base Z", min: -6, max: 6, step: 0.1, value: 0 },
  ...baseSweepControls(0x2ecc71, true, true),
];

bootstrapSweepDemo({
  title: "Sweep Usage: Simple Pipe",
  description: "Straight vertical sweep with rectangular profile and configurable caps.",
  panelTitle: "Simple Pipe Parameters",
  controls,
  cameraPosition: [8.6, 7.4, 9.8],
  controlsTarget: [0, 4, 0],
  createSceneObject: ({ state, THREE }) => {
    const path = buildPath(state.height, state.baseX, state.baseZ);
    const profile = createSquareProfile(state.profileWidth, state.profileDepth);

    const pipe = new Sweep({
      path,
      profile,
      color: state.color,
      capStart: state.capStart,
      capEnd: state.capEnd,
      outlineWidth: state.outlineWidth,
      fatOutlines: state.fatOutlines,
    });
    applySweepAppearance(pipe, state);

    const group = new THREE.Group();
    const pathGuide = createPathGuide(path, 0x4b5563);
    pathGuide.position.y += 0.01;

    const profileGuide = createProfileGuide(profile, new THREE.Vector3(-4.2, 0, -3.4), 0x0f172a);

    group.add(pipe);
    group.add(pathGuide);
    group.add(profileGuide);
    return group;
  },
});
