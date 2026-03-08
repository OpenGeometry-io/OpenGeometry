import { Vector3 } from "../../../../../dist/index.js";

export function clampInt(value, min, max) {
  return Math.min(max, Math.max(min, Math.round(value)));
}

export function createSquareProfile(width, depth) {
  return [
    new Vector3(-width * 0.5, 0, -depth * 0.5),
    new Vector3(width * 0.5, 0, -depth * 0.5),
    new Vector3(width * 0.5, 0, depth * 0.5),
    new Vector3(-width * 0.5, 0, depth * 0.5),
  ];
}

export function createCircularProfile(radius, segments) {
  const clamped = clampInt(segments, 3, 128);
  const result = [];

  for (let i = 0; i < clamped; i += 1) {
    const angle = (i / clamped) * Math.PI * 2;
    result.push(new Vector3(Math.cos(angle) * radius, 0, Math.sin(angle) * radius));
  }

  return result;
}

export function applySweepAppearance(sweep, state) {
  sweep.outline = Boolean(state.outline);
  sweep.fatOutlines = Boolean(state.fatOutlines);
  sweep.outlineWidth = Number(state.outlineWidth);
}

export function baseSweepControls(defaultColor, defaultCapStart, defaultCapEnd) {
  return [
    { type: "boolean", key: "capStart", label: "Cap Start", value: defaultCapStart },
    { type: "boolean", key: "capEnd", label: "Cap End", value: defaultCapEnd },
    { type: "boolean", key: "outline", label: "Outline", value: true },
    { type: "boolean", key: "fatOutlines", label: "Fat Outlines", value: false },
    {
      type: "number",
      key: "outlineWidth",
      label: "Outline Width",
      min: 1,
      max: 12,
      step: 0.5,
      value: 4,
    },
    { type: "color", key: "color", label: "Sweep Color", value: defaultColor },
  ];
}
