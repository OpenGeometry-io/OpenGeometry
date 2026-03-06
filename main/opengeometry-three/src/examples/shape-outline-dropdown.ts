import * as THREE from "three";
import type { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { Vector3 } from "../../../opengeometry/pkg/opengeometry";
import { Cuboid } from "../shapes/cuboid";
import { Cylinder } from "../shapes/cylinder";
import { Opening } from "../shapes/opening";
import { Polygon } from "../shapes/polygon";
import { Sphere } from "../shapes/sphere";
import { Sweep } from "../shapes/sweep";
import { Wedge } from "../shapes/wedge";

export type OutlineRefreshMode = "live" | "on-stop" | "manual";

type ShapeKey =
  | "polygon"
  | "cuboid"
  | "cylinder"
  | "wedge"
  | "opening"
  | "sweep"
  | "sphere";

type ShapeObject = THREE.Object3D & {
  outline?: boolean;
  getHlrOutlineGeometry?: Function;
};

export interface ShapeOutlineDropdownOptions {
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  renderer: THREE.WebGLRenderer;
  controls: OrbitControls;
  initialShape?: ShapeKey;
  initialRefreshMode?: OutlineRefreshMode;
  initialOutlineEnabled?: boolean;
}

export interface ShapeOutlineDropdownHandle {
  dispose: () => void;
  refresh: () => void;
}

const SHAPE_OPTIONS: ReadonlyArray<{ key: ShapeKey; label: string }> = [
  { key: "polygon", label: "Polygon" },
  { key: "cuboid", label: "Cuboid" },
  { key: "cylinder", label: "Cylinder" },
  { key: "wedge", label: "Wedge" },
  { key: "opening", label: "Opening" },
  { key: "sweep", label: "Sweep" },
  { key: "sphere", label: "Sphere" },
];

const HLR_DEBOUNCE_MS = 120;

function isShapeKey(value: string): value is ShapeKey {
  return SHAPE_OPTIONS.some((entry) => entry.key === value);
}

function ensureOutlineControlStyles(): void {
  if (document.getElementById("og-outline-controls-style")) {
    return;
  }

  const style = document.createElement("style");
  style.id = "og-outline-controls-style";
  style.textContent = `
    .og-outline-controls {
      position: fixed;
      top: 12px;
      right: 12px;
      width: min(320px, calc(100vw - 24px));
      padding: 12px;
      border: 1px solid rgba(17, 24, 39, 0.16);
      background: rgba(250, 250, 250, 0.95);
      border-radius: 10px;
      box-shadow: 0 8px 24px rgba(15, 23, 42, 0.14);
      z-index: 6;
      display: grid;
      gap: 10px;
      backdrop-filter: blur(4px);
    }

    .og-outline-controls h3 {
      margin: 0;
      font-size: 13px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: #334155;
    }

    .og-outline-controls .og-control-row {
      display: grid;
      gap: 6px;
    }

    .og-outline-controls .og-control-label {
      font-size: 11px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: #64748b;
    }

    .og-outline-controls .og-control-select,
    .og-outline-controls .og-control-button {
      width: 100%;
      border: 1px solid rgba(100, 116, 139, 0.35);
      border-radius: 6px;
      min-height: 32px;
      padding: 6px 10px;
      background: #f8fafc;
      color: #0f172a;
      font-size: 12px;
      font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
    }

    .og-outline-controls .og-control-button {
      cursor: pointer;
    }

    .og-outline-controls .og-control-button[hidden] {
      display: none;
    }

    .og-outline-controls .og-control-bool {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .og-outline-controls .og-toggle {
      -webkit-appearance: none;
      appearance: none;
      width: 42px;
      height: 22px;
      margin: 0;
      border: 1px solid #9ca3af;
      border-radius: 999px;
      background: #d1d5db;
      position: relative;
      cursor: pointer;
      transition: background-color 140ms ease, border-color 140ms ease;
    }

    .og-outline-controls .og-toggle::after {
      content: "";
      position: absolute;
      top: 2px;
      left: 2px;
      width: 16px;
      height: 16px;
      border-radius: 999px;
      background: #ffffff;
      box-shadow: inset 0 0 0 1px #9ca3af;
      transition: left 140ms ease;
    }

    .og-outline-controls .og-toggle:checked {
      background: #bfdbfe;
      border-color: #60a5fa;
    }

    .og-outline-controls .og-toggle:checked::after {
      left: calc(100% - 18px);
    }

    .og-outline-controls .og-control-bool-state {
      font-size: 11px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: #64748b;
    }
  `;

  document.head.appendChild(style);
}

function disposeMaterial(material: unknown): void {
  if (!material) {
    return;
  }

  if (Array.isArray(material)) {
    material.forEach((entry) => {
      const candidate = entry as { dispose?: () => void };
      candidate.dispose?.();
    });
    return;
  }

  const candidate = material as { dispose?: () => void };
  candidate.dispose?.();
}

function disposeObject3D(object: THREE.Object3D): void {
  object.traverse((node) => {
    const withGeometry = node as { geometry?: { dispose?: () => void } };
    const withMaterial = node as { material?: unknown };

    withGeometry.geometry?.dispose?.();
    disposeMaterial(withMaterial.material);
  });
}

function createShape(key: ShapeKey): ShapeObject {
  switch (key) {
    case "polygon":
      return new Polygon({
        vertices: [
          new Vector3(-1.6, 0.0, -0.8),
          new Vector3(-0.7, 0.0, 1.1),
          new Vector3(0.6, 0.0, 1.0),
          new Vector3(1.4, 0.0, -0.2),
          new Vector3(0.2, 0.0, -1.2),
        ],
        color: 0x2563eb,
      });
    case "cuboid":
      return new Cuboid({
        center: new Vector3(0.0, 0.9, 0.0),
        width: 2.0,
        height: 1.8,
        depth: 1.5,
        color: 0x10b981,
      });
    case "cylinder":
      return new Cylinder({
        center: new Vector3(0.0, 0.9, 0.0),
        radius: 0.85,
        height: 1.8,
        segments: 40,
        angle: Math.PI * 2,
        color: 0xf97316,
      });
    case "wedge":
      return new Wedge({
        center: new Vector3(0.0, 0.8, 0.0),
        width: 2.2,
        height: 1.6,
        depth: 1.6,
        color: 0x7c3aed,
      });
    case "opening":
      return new Opening({
        center: new Vector3(0.0, 1.0, 0.0),
        width: 1.2,
        height: 2.0,
        depth: 0.4,
        color: 0x94a3b8,
      });
    case "sweep":
      return new Sweep({
        path: [
          new Vector3(-1.8, 0.0, -0.4),
          new Vector3(-1.1, 0.5, 0.8),
          new Vector3(-0.1, 1.1, 1.0),
          new Vector3(0.9, 1.8, 0.2),
          new Vector3(1.7, 2.2, -0.9),
        ],
        profile: [
          new Vector3(-0.22, 0.0, -0.22),
          new Vector3(0.22, 0.0, -0.22),
          new Vector3(0.22, 0.0, 0.22),
          new Vector3(-0.22, 0.0, 0.22),
        ],
        color: 0x14b8a6,
        capStart: true,
        capEnd: true,
      });
    case "sphere":
      return new Sphere({
        center: new Vector3(0.0, 1.0, 0.0),
        radius: 1.0,
        widthSegments: 32,
        heightSegments: 20,
        color: 0x0891b2,
      });
    default:
      throw new Error(`Unsupported shape key: ${key}`);
  }
}

export function createShapeOutlineDropdownExample(
  options: ShapeOutlineDropdownOptions
): ShapeOutlineDropdownHandle {
  ensureOutlineControlStyles();

  const initialShape = options.initialShape ?? "cuboid";
  const initialRefreshMode = options.initialRefreshMode ?? "live";
  const initialOutlineEnabled = options.initialOutlineEnabled ?? true;

  const panel = document.createElement("aside");
  panel.className = "og-controls og-outline-controls";

  const heading = document.createElement("h3");
  heading.textContent = "CAD Outline";
  panel.appendChild(heading);

  const shapeRow = document.createElement("label");
  shapeRow.className = "og-control-row";

  const shapeLabelElement = document.createElement("span");
  shapeLabelElement.className = "og-control-label";
  shapeLabelElement.textContent = "Shape";
  shapeRow.appendChild(shapeLabelElement);

  const shapeSelect = document.createElement("select");
  shapeSelect.className = "og-control-select";
  for (const option of SHAPE_OPTIONS) {
    const entry = document.createElement("option");
    entry.value = option.key;
    entry.textContent = option.label;
    if (option.key === initialShape) {
      entry.selected = true;
    }
    shapeSelect.appendChild(entry);
  }
  shapeRow.appendChild(shapeSelect);
  panel.appendChild(shapeRow);

  const outlineRow = document.createElement("label");
  outlineRow.className = "og-control-row";

  const outlineHeader = document.createElement("div");
  outlineHeader.className = "og-control-header";

  const outlineLabel = document.createElement("span");
  outlineLabel.className = "og-control-label";
  outlineLabel.textContent = "HLR Outline";
  outlineHeader.appendChild(outlineLabel);
  outlineRow.appendChild(outlineHeader);

  const outlineToggleWrap = document.createElement("div");
  outlineToggleWrap.className = "og-control-bool";

  const outlineToggle = document.createElement("input");
  outlineToggle.type = "checkbox";
  outlineToggle.className = "og-toggle";
  outlineToggle.checked = initialOutlineEnabled;
  outlineToggle.setAttribute("aria-label", "Toggle HLR outline");

  const outlineState = document.createElement("span");
  outlineState.className = "og-control-bool-state";
  outlineState.textContent = initialOutlineEnabled ? "Enabled" : "Disabled";

  outlineToggleWrap.appendChild(outlineToggle);
  outlineToggleWrap.appendChild(outlineState);
  outlineRow.appendChild(outlineToggleWrap);
  panel.appendChild(outlineRow);

  const refreshModeRow = document.createElement("label");
  refreshModeRow.className = "og-control-row";

  const refreshModeLabel = document.createElement("span");
  refreshModeLabel.className = "og-control-label";
  refreshModeLabel.textContent = "Refresh";
  refreshModeRow.appendChild(refreshModeLabel);

  const refreshModeSelect = document.createElement("select");
  refreshModeSelect.className = "og-control-select";
  [
    { value: "live", label: "Live" },
    { value: "on-stop", label: "On Stop" },
    { value: "manual", label: "Manual" },
  ].forEach((entry) => {
    const option = document.createElement("option");
    option.value = entry.value;
    option.textContent = entry.label;
    if (entry.value === initialRefreshMode) {
      option.selected = true;
    }
    refreshModeSelect.appendChild(option);
  });
  refreshModeRow.appendChild(refreshModeSelect);
  panel.appendChild(refreshModeRow);

  const refreshButton = document.createElement("button");
  refreshButton.type = "button";
  refreshButton.className = "og-control-button";
  refreshButton.textContent = "Refresh Outline";
  panel.appendChild(refreshButton);

  document.body.appendChild(panel);

  let currentShape: ShapeObject | null = null;
  let currentShapeKey: ShapeKey = initialShape;
  let refreshMode: OutlineRefreshMode = initialRefreshMode;
  let outlineEnabled = initialOutlineEnabled;
  let outlineLines: THREE.LineSegments | null = null;
  let pendingTimer: number | null = null;
  let renderSignature = "";

  const removeOutline = () => {
    renderSignature = "";
    if (!outlineLines) {
      return;
    }

    outlineLines.parent?.remove(outlineLines);
    outlineLines.geometry.dispose();
    if (outlineLines.material instanceof THREE.Material) {
      outlineLines.material.dispose();
    }
    outlineLines = null;
  };

  const applyOutlineData = (lineBuffer: number[]) => {
    if (!outlineEnabled || lineBuffer.length === 0) {
      removeOutline();
      return;
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(lineBuffer, 3));

    if (!outlineLines) {
      const material = new THREE.LineBasicMaterial({
        color: 0x111111,
        depthTest: false,
        depthWrite: false,
      });

      outlineLines = new THREE.LineSegments(geometry, material);
      outlineLines.renderOrder = 10;
      options.scene.add(outlineLines);
      return;
    }

    outlineLines.geometry.dispose();
    outlineLines.geometry = geometry;
  };

  const updateOutline = (force = false) => {
    if (!currentShape) {
      removeOutline();
      return;
    }

    if (!outlineEnabled) {
      removeOutline();
      return;
    }

    const nextSignature = [
      currentShapeKey,
      options.camera.position.x,
      options.camera.position.y,
      options.camera.position.z,
      options.controls.target.x,
      options.controls.target.y,
      options.controls.target.z,
      options.camera.up.x,
      options.camera.up.y,
      options.camera.up.z,
      options.camera.near,
    ]
      .map((value) => Number(value).toFixed(6))
      .join("|");

    if (!force && nextSignature === renderSignature) {
      return;
    }

    if (typeof currentShape.getHlrOutlineGeometry !== "function") {
      removeOutline();
      return;
    }

    const lineBuffer = currentShape.getHlrOutlineGeometry(
      options.camera,
      options.controls.target,
      true
    ) as number[];

    applyOutlineData(lineBuffer);
    renderSignature = nextSignature;
  };

  const scheduleOnStop = () => {
    if (pendingTimer !== null) {
      window.clearTimeout(pendingTimer);
    }

    pendingTimer = window.setTimeout(() => {
      pendingTimer = null;
      updateOutline();
    }, HLR_DEBOUNCE_MS);
  };

  const mountShape = (shape: ShapeObject) => {
    if (currentShape) {
      currentShape.parent?.remove(currentShape);
      disposeObject3D(currentShape);
    }

    currentShape = shape;

    if ("outline" in currentShape) {
      currentShape.outline = false;
    }

    options.scene.add(currentShape);
    updateOutline(true);
  };

  const rebuildShape = () => {
    const shape = createShape(currentShapeKey);
    mountShape(shape);
  };

  const onShapeChange = () => {
    const nextValue = shapeSelect.value;
    if (!isShapeKey(nextValue)) {
      return;
    }

    currentShapeKey = nextValue;
    rebuildShape();
  };

  const onOutlineToggleChange = () => {
    outlineEnabled = outlineToggle.checked;
    outlineState.textContent = outlineEnabled ? "Enabled" : "Disabled";

    if (currentShape && "outline" in currentShape) {
      currentShape.outline = false;
    }

    if (!outlineEnabled) {
      removeOutline();
      return;
    }

    updateOutline(true);
  };

  const updateManualButtonVisibility = () => {
    const isManual = refreshMode === "manual";
    refreshButton.hidden = !isManual;
    refreshButton.disabled = !isManual;
  };

  const onRefreshModeChange = () => {
    refreshMode = refreshModeSelect.value as OutlineRefreshMode;
    updateManualButtonVisibility();
    updateOutline(true);
  };

  const onControlsChange = () => {
    if (!outlineEnabled) {
      return;
    }

    if (refreshMode === "live") {
      updateOutline();
      return;
    }

    if (refreshMode === "on-stop") {
      scheduleOnStop();
    }
  };

  const onControlsEnd = () => {
    if (!outlineEnabled || refreshMode !== "on-stop") {
      return;
    }

    if (pendingTimer !== null) {
      window.clearTimeout(pendingTimer);
      pendingTimer = null;
    }

    updateOutline(true);
  };

  shapeSelect.addEventListener("change", onShapeChange);
  outlineToggle.addEventListener("change", onOutlineToggleChange);
  refreshModeSelect.addEventListener("change", onRefreshModeChange);
  refreshButton.addEventListener("click", () => updateOutline(true));
  options.controls.addEventListener("change", onControlsChange);
  options.controls.addEventListener("end", onControlsEnd);

  updateManualButtonVisibility();
  rebuildShape();

  return {
    dispose: () => {
      shapeSelect.removeEventListener("change", onShapeChange);
      outlineToggle.removeEventListener("change", onOutlineToggleChange);
      refreshModeSelect.removeEventListener("change", onRefreshModeChange);
      options.controls.removeEventListener("change", onControlsChange);
      options.controls.removeEventListener("end", onControlsEnd);

      if (pendingTimer !== null) {
        window.clearTimeout(pendingTimer);
        pendingTimer = null;
      }

      if (currentShape) {
        currentShape.parent?.remove(currentShape);
        disposeObject3D(currentShape);
        currentShape = null;
      }

      removeOutline();
      panel.remove();
    },
    refresh: () => {
      updateOutline(true);
    },
  };
}

export function getAvailableOutlineShapes(): ReadonlyArray<{ key: ShapeKey; label: string }> {
  return SHAPE_OPTIONS;
}
