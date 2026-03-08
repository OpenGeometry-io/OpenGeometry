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

function buildCurvedPath(radius, height, arcDegrees, samples) {
  const count = clampInt(samples, 4, 96);
  const arc = (arcDegrees * Math.PI) / 180;
  const points = [];

  for (let i = 0; i <= count; i += 1) {
    const t = i / count;
    const angle = t * arc;
    points.push(new Vector3(Math.cos(angle) * radius, t * height, Math.sin(angle) * radius));
  }

  return points;
}

const controls = [
  { type: "number", key: "pathRadius", label: "Path Radius", min: 1, max: 9, step: 0.1, value: 5 },
  { type: "number", key: "pathHeight", label: "Path Height", min: 2, max: 20, step: 0.2, value: 10 },
  { type: "number", key: "arcDegrees", label: "Arc Degrees", min: 45, max: 360, step: 1, value: 180 },
  { type: "number", key: "pathSamples", label: "Path Samples", min: 4, max: 80, step: 1, value: 20 },
  {
    type: "number",
    key: "profileRadius",
    label: "Profile Radius",
    min: 0.05,
    max: 1.8,
    step: 0.05,
    value: 0.5,
  },
  {
    type: "number",
    key: "profileSegments",
    label: "Profile Segments",
    min: 3,
    max: 64,
    step: 1,
    value: 12,
  },
  ...baseSweepControls(0x3498db, true, true),
];

bootstrapSweepDemo({
  title: "Sweep Usage: Curved Tube",
  description: "Arc-driven path sweep with circular profile controls.",
  panelTitle: "Curved Tube Parameters",
  controls,
  cameraPosition: [13.5, 9.5, 13.5],
  controlsTarget: [0, 5, 0],
  createSceneObject: ({ state, THREE }) => {
    const path = buildCurvedPath(state.pathRadius, state.pathHeight, state.arcDegrees, state.pathSamples);
    const profile = createCircularProfile(state.profileRadius, state.profileSegments);

    const tube = new Sweep({
      path,
      profile,
      color: state.color,
      capStart: state.capStart,
      capEnd: state.capEnd,
      outlineWidth: state.outlineWidth,
      fatOutlines: state.fatOutlines,
    });
    applySweepAppearance(tube, state);

    const group = new THREE.Group();
    const pathGuide = createPathGuide(path, 0x334155);
    pathGuide.position.y += 0.01;

    const profileGuide = createProfileGuide(profile, new THREE.Vector3(-6.2, 0, -6.2), 0x0f172a);

    group.add(tube);
    group.add(pathGuide);
    group.add(profileGuide);
    return group;
  },
});
