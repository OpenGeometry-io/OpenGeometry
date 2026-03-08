import { Sweep, Vector3 } from "../../../../../dist/index.js";
import {
  bootstrapSweepDemo,
  createPathGuide,
  createProfileGuide,
} from "../shared/demo-runtime.js";
import {
  applySweepAppearance,
  baseSweepControls,
  clampInt,
  createCircularProfile,
} from "./common.js";

function buildHandrailPath(stepCount, stepRise, stepRun, offsetX, startHeight) {
  const steps = clampInt(stepCount, 2, 20);
  const points = [];

  for (let i = 0; i <= steps; i += 1) {
    points.push(new Vector3(offsetX, startHeight + i * stepRise, i * stepRun));
  }

  return points;
}

const controls = [
  { type: "number", key: "stepCount", label: "Step Count", min: 2, max: 16, step: 1, value: 4 },
  { type: "number", key: "stepRise", label: "Step Rise", min: 0.15, max: 3, step: 0.05, value: 2 },
  { type: "number", key: "stepRun", label: "Step Run", min: 0.15, max: 3, step: 0.05, value: 2 },
  { type: "number", key: "offsetX", label: "Lateral Offset", min: -4, max: 4, step: 0.1, value: 0 },
  { type: "number", key: "startHeight", label: "Start Height", min: -1, max: 4, step: 0.1, value: 0 },
  {
    type: "number",
    key: "profileRadius",
    label: "Rail Radius",
    min: 0.05,
    max: 1.4,
    step: 0.05,
    value: 0.3,
  },
  {
    type: "number",
    key: "profileSegments",
    label: "Rail Segments",
    min: 4,
    max: 64,
    step: 1,
    value: 16,
  },
  ...baseSweepControls(0x8b4513, true, true),
];

bootstrapSweepDemo({
  title: "Sweep Usage: Handrail",
  description: "Staircase-like path sweep for railing studies and detailing.",
  panelTitle: "Handrail Parameters",
  controls,
  cameraPosition: [10, 7.2, 13],
  controlsTarget: [0, 4.4, 4],
  createSceneObject: ({ state, THREE }) => {
    const path = buildHandrailPath(
      state.stepCount,
      state.stepRise,
      state.stepRun,
      state.offsetX,
      state.startHeight,
    );
    const profile = createCircularProfile(state.profileRadius, state.profileSegments);

    const handrail = new Sweep({
      path,
      profile,
      color: state.color,
      capStart: state.capStart,
      capEnd: state.capEnd,
      outlineWidth: state.outlineWidth,
      fatOutlines: state.fatOutlines,
    });
    applySweepAppearance(handrail, state);

    const group = new THREE.Group();
    const pathGuide = createPathGuide(path, 0x64748b);
    pathGuide.position.y += 0.01;

    const previewOffset = -2.4 - state.profileRadius;
    const profileGuide = createProfileGuide(profile, new THREE.Vector3(previewOffset, 0, -2.8), 0x0f172a);

    group.add(handrail);
    group.add(pathGuide);
    group.add(profileGuide);
    return group;
  },
});
