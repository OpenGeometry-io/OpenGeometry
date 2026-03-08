import { Sweep, Vector3 } from "../../../../../dist/index.js";
import {
  bootstrapSweepDemo,
  createPathGuide,
  createProfileGuide,
} from "../shared/demo-runtime.js";
import {
  applySweepAppearance,
  baseSweepControls,
} from "./common.js";

function buildMoldingProfile(widthScale, depthScale, profileOffset) {
  return [
    new Vector3(0 + profileOffset, 0, 0),
    new Vector3(0.2 * widthScale + profileOffset, 0, 0.1 * depthScale),
    new Vector3(0.3 * widthScale + profileOffset, 0, 0.1 * depthScale),
    new Vector3(0.4 * widthScale + profileOffset, 0, 0.3 * depthScale),
    new Vector3(0.5 * widthScale + profileOffset, 0, 0.3 * depthScale),
    new Vector3(0.6 * widthScale + profileOffset, 0, 0),
  ];
}

function buildRoomPath(lengthX, lengthZ, elevation, startOffset) {
  return [
    new Vector3(startOffset, elevation, 0),
    new Vector3(lengthX + startOffset, elevation, 0),
    new Vector3(lengthX + startOffset, elevation, lengthZ),
  ];
}

const controls = [
  { type: "number", key: "lengthX", label: "Room Leg X", min: 2, max: 24, step: 0.2, value: 10 },
  { type: "number", key: "lengthZ", label: "Room Leg Z", min: 2, max: 24, step: 0.2, value: 10 },
  { type: "number", key: "elevation", label: "Elevation", min: 0, max: 20, step: 0.2, value: 8 },
  { type: "number", key: "startOffset", label: "Path Start Offset", min: -6, max: 6, step: 0.1, value: 0 },
  {
    type: "number",
    key: "profileWidthScale",
    label: "Profile Width Scale",
    min: 0.2,
    max: 3,
    step: 0.05,
    value: 1,
  },
  {
    type: "number",
    key: "profileDepthScale",
    label: "Profile Depth Scale",
    min: 0.2,
    max: 3,
    step: 0.05,
    value: 1,
  },
  {
    type: "number",
    key: "profileOffset",
    label: "Profile Offset",
    min: -1,
    max: 1,
    step: 0.02,
    value: 0,
  },
  ...baseSweepControls(0xf5f5dc, false, false),
];

bootstrapSweepDemo({
  title: "Sweep Usage: Architectural Molding",
  description: "Decorative profile sweep around a room corner at set elevation.",
  panelTitle: "Architectural Molding Parameters",
  controls,
  cameraPosition: [16, 14, 18],
  controlsTarget: [6, 8, 4],
  createSceneObject: ({ state, THREE }) => {
    const path = buildRoomPath(state.lengthX, state.lengthZ, state.elevation, state.startOffset);
    const profile = buildMoldingProfile(state.profileWidthScale, state.profileDepthScale, state.profileOffset);

    const molding = new Sweep({
      path,
      profile,
      color: state.color,
      capStart: state.capStart,
      capEnd: state.capEnd,
      outlineWidth: state.outlineWidth,
      fatOutlines: state.fatOutlines,
    });
    applySweepAppearance(molding, state);

    const group = new THREE.Group();
    const pathGuide = createPathGuide(path, 0x475569);
    pathGuide.position.y += 0.01;

    const profileGuide = createProfileGuide(profile, new THREE.Vector3(-3.4, 0, -3.2), 0x0f172a);

    group.add(molding);
    group.add(pathGuide);
    group.add(profileGuide);
    return group;
  },
});
