import { Cuboid, Cylinder, OGSceneManager, Sphere, Vector3, Wedge } from "@og-three";
import * as THREE from "three";
import { bootstrapExample, replaceSceneObject } from "../shared/runtime";

interface StlExportReport {
  input_breps: number;
  input_faces: number;
  exported_triangles: number;
  skipped_faces: number;
  skipped_triangles: number;
  topology_errors: number;
}

type ShapeKind = "cuboid" | "cylinder" | "sphere" | "wedge";

type NumberKey =
  | "cuboidWidth"
  | "cuboidHeight"
  | "cuboidDepth"
  | "cylinderRadius"
  | "cylinderHeight"
  | "cylinderSegments"
  | "cylinderAngleDeg"
  | "sphereRadius"
  | "sphereWidthSegments"
  | "sphereHeightSegments"
  | "wedgeWidth"
  | "wedgeHeight"
  | "wedgeDepth";

interface NumberControlDef {
  key: NumberKey;
  label: string;
  min: number;
  max: number;
  step: number;
  integer?: boolean;
}

const SHAPE_LABELS: Record<ShapeKind, string> = {
  cuboid: "Cuboid",
  cylinder: "Cylinder",
  sphere: "Sphere",
  wedge: "Wedge",
};

const SHAPE_CONTROL_DEFS: Record<ShapeKind, NumberControlDef[]> = {
  cuboid: [
    { key: "cuboidWidth", label: "Width", min: 0.2, max: 4.0, step: 0.05 },
    { key: "cuboidHeight", label: "Height", min: 0.2, max: 4.0, step: 0.05 },
    { key: "cuboidDepth", label: "Depth", min: 0.2, max: 4.0, step: 0.05 },
  ],
  cylinder: [
    { key: "cylinderRadius", label: "Radius", min: 0.2, max: 3.0, step: 0.05 },
    { key: "cylinderHeight", label: "Height", min: 0.2, max: 4.0, step: 0.05 },
    { key: "cylinderSegments", label: "Segments", min: 6, max: 96, step: 1, integer: true },
    { key: "cylinderAngleDeg", label: "Angle (deg)", min: 20, max: 360, step: 1, integer: true },
  ],
  sphere: [
    { key: "sphereRadius", label: "Radius", min: 0.2, max: 3.0, step: 0.05 },
    { key: "sphereWidthSegments", label: "Width Segments", min: 3, max: 96, step: 1, integer: true },
    { key: "sphereHeightSegments", label: "Height Segments", min: 2, max: 64, step: 1, integer: true },
  ],
  wedge: [
    { key: "wedgeWidth", label: "Width", min: 0.2, max: 4.0, step: 0.05 },
    { key: "wedgeHeight", label: "Height", min: 0.2, max: 4.0, step: 0.05 },
    { key: "wedgeDepth", label: "Depth", min: 0.2, max: 4.0, step: 0.05 },
  ],
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function downloadStl(bytes: Uint8Array, filename: string) {
  const blob = new Blob([bytes], { type: "model/stl" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

function formatReport(report: StlExportReport, bytes: number, filename: string): string {
  return [
    `File: ${filename}`,
    `Bytes: ${bytes}`,
    `Input BReps: ${report.input_breps}`,
    `Input Faces: ${report.input_faces}`,
    `Exported Triangles: ${report.exported_triangles}`,
    `Skipped Faces: ${report.skipped_faces}`,
    `Skipped Triangles: ${report.skipped_triangles}`,
    `Topology Errors: ${report.topology_errors}`,
  ].join("\n");
}

bootstrapExample({
  title: "Operation: STL Export",
  description:
    "Select a shape, tune parameters, and export a binary STL with best-effort reporting.",
  build: ({ scene }) => {
    const manager = new OGSceneManager();
    const sceneId = manager.createScene("stl-export-scene");
    const entityId = "stl-shape";

    let current: THREE.Object3D | null = null;

    const state: {
      shape: ShapeKind;
      outline: boolean;
      values: Record<NumberKey, number>;
    } = {
      shape: "cuboid",
      outline: true,
      values: {
        cuboidWidth: 1.8,
        cuboidHeight: 1.4,
        cuboidDepth: 1.1,
        cylinderRadius: 0.8,
        cylinderHeight: 1.8,
        cylinderSegments: 36,
        cylinderAngleDeg: 360,
        sphereRadius: 1.0,
        sphereWidthSegments: 32,
        sphereHeightSegments: 20,
        wedgeWidth: 2.0,
        wedgeHeight: 1.8,
        wedgeDepth: 1.4,
      },
    };

    const panel = document.createElement("aside");
    panel.className = "og-controls";

    const heading = document.createElement("h3");
    heading.textContent = "STL Export";
    panel.appendChild(heading);

    const summary = document.createElement("p");
    summary.style.margin = "0 0 0.75rem";
    summary.style.fontSize = "0.75rem";
    summary.style.color = "#5f5f5f";
    summary.textContent = "Pick a shape and export the currently displayed model as Binary STL.";
    panel.appendChild(summary);

    const shapeRow = document.createElement("label");
    shapeRow.className = "og-control-row";

    const shapeHeader = document.createElement("div");
    shapeHeader.className = "og-control-header";

    const shapeLabel = document.createElement("span");
    shapeLabel.className = "og-control-label";
    shapeLabel.textContent = "Shape";
    shapeHeader.appendChild(shapeLabel);

    shapeRow.appendChild(shapeHeader);

    const shapeSelectWrap = document.createElement("div");
    shapeSelectWrap.style.display = "grid";
    shapeSelectWrap.style.gridTemplateColumns = "1fr";

    const shapeSelect = document.createElement("select");
    shapeSelect.style.width = "100%";
    shapeSelect.style.border = "1px solid #b8b8b8";
    shapeSelect.style.background = "#dfdfdf";
    shapeSelect.style.color = "#242424";
    shapeSelect.style.padding = "6px";
    shapeSelect.style.font = '11px/1.2 "IBM Plex Mono", "JetBrains Mono", "SFMono-Regular", monospace';

    (Object.keys(SHAPE_LABELS) as ShapeKind[]).forEach((shape) => {
      const option = document.createElement("option");
      option.value = shape;
      option.textContent = SHAPE_LABELS[shape];
      shapeSelect.appendChild(option);
    });
    shapeSelect.value = state.shape;

    shapeSelectWrap.appendChild(shapeSelect);
    shapeRow.appendChild(shapeSelectWrap);
    panel.appendChild(shapeRow);

    const shapeGroups = new Map<ShapeKind, HTMLDivElement>();

    const renderCurrentShape = () => {
      try {
        let object: THREE.Object3D;
        let kind: string;
        let brepData: unknown;
        let fileName: string;
        let readyText: string;

        switch (state.shape) {
          case "cuboid": {
            const width = state.values.cuboidWidth;
            const height = state.values.cuboidHeight;
            const depth = state.values.cuboidDepth;

            const cuboid = new Cuboid({
              center: new Vector3(0, height * 0.5, 0),
              width,
              height,
              depth,
              color: 0x0ea5e9,
            });
            cuboid.outline = state.outline;
            object = cuboid;
            kind = "OGCuboid";
            brepData = cuboid.getBrepData();
            fileName = "opengeometry-cuboid.stl";
            readyText = `Ready: Cuboid (W=${width.toFixed(2)}, H=${height.toFixed(2)}, D=${depth.toFixed(2)})`;
            break;
          }
          case "cylinder": {
            const radius = state.values.cylinderRadius;
            const height = state.values.cylinderHeight;
            const segments = Math.max(3, Math.floor(state.values.cylinderSegments));
            const angleDeg = Math.floor(state.values.cylinderAngleDeg);

            const cylinder = new Cylinder({
              center: new Vector3(0, height * 0.5, 0),
              radius,
              height,
              segments,
              angle: (angleDeg * Math.PI) / 180,
              color: 0xf59e0b,
            });
            cylinder.outline = state.outline;
            object = cylinder;
            kind = "OGCylinder";
            brepData = cylinder.getBrep();
            fileName = "opengeometry-cylinder.stl";
            readyText = `Ready: Cylinder (R=${radius.toFixed(2)}, H=${height.toFixed(2)}, Seg=${segments}, Angle=${angleDeg}deg)`;
            break;
          }
          case "sphere": {
            const radius = state.values.sphereRadius;
            const widthSegments = Math.max(3, Math.floor(state.values.sphereWidthSegments));
            const heightSegments = Math.max(2, Math.floor(state.values.sphereHeightSegments));

            const sphere = new Sphere({
              center: new Vector3(0, radius, 0),
              radius,
              widthSegments,
              heightSegments,
              color: 0x0891b2,
            });
            sphere.outline = state.outline;
            object = sphere;
            kind = "OGSphere";
            brepData = sphere.getBrep();
            fileName = "opengeometry-sphere.stl";
            readyText = `Ready: Sphere (R=${radius.toFixed(2)}, WSeg=${widthSegments}, HSeg=${heightSegments})`;
            break;
          }
          case "wedge": {
            const width = state.values.wedgeWidth;
            const height = state.values.wedgeHeight;
            const depth = state.values.wedgeDepth;

            const wedge = new Wedge({
              center: new Vector3(0, height * 0.5, 0),
              width,
              height,
              depth,
              color: 0x7c3aed,
            });
            wedge.outline = state.outline;
            object = wedge;
            kind = "OGWedge";
            brepData = wedge.getBrepData();
            fileName = "opengeometry-wedge.stl";
            readyText = `Ready: Wedge (W=${width.toFixed(2)}, H=${height.toFixed(2)}, D=${depth.toFixed(2)})`;
            break;
          }
        }

        current = replaceSceneObject(scene, current, object);
        manager.addBrepEntityToScene(
          sceneId,
          entityId,
          kind,
          JSON.stringify(brepData)
        );

        exportButton.dataset.filename = fileName;
        reportNode.textContent = readyText;
      } catch (error) {
        reportNode.textContent = `Shape update failed: ${String(error)}`;
      }
    };

    const createNumberRow = (group: HTMLElement, def: NumberControlDef) => {
      const row = document.createElement("label");
      row.className = "og-control-row";

      const header = document.createElement("div");
      header.className = "og-control-header";

      const label = document.createElement("span");
      label.className = "og-control-label";
      label.textContent = def.label;
      header.appendChild(label);

      const valueLabel = document.createElement("code");
      valueLabel.className = "og-control-value";
      header.appendChild(valueLabel);
      row.appendChild(header);

      const inputs = document.createElement("div");
      inputs.className = "og-control-range";

      const rangeInput = document.createElement("input");
      rangeInput.type = "range";
      rangeInput.min = String(def.min);
      rangeInput.max = String(def.max);
      rangeInput.step = String(def.step);

      const numberInput = document.createElement("input");
      numberInput.type = "number";
      numberInput.min = String(def.min);
      numberInput.max = String(def.max);
      numberInput.step = String(def.step);

      const syncDisplay = (value: number) => {
        state.values[def.key] = value;
        rangeInput.value = String(value);
        numberInput.value = String(value);
        valueLabel.textContent = def.integer
          ? `${Math.round(value)}`
          : value.toFixed(3).replace(/\.?0+$/, "");
      };

      const updateValue = (raw: number) => {
        const constrained = clamp(raw, def.min, def.max);
        const nextValue = def.integer ? Math.round(constrained) : constrained;
        syncDisplay(nextValue);
        renderCurrentShape();
      };

      syncDisplay(state.values[def.key]);

      rangeInput.addEventListener("input", () => {
        updateValue(Number(rangeInput.value));
      });
      numberInput.addEventListener("change", () => {
        updateValue(Number(numberInput.value));
      });

      inputs.appendChild(rangeInput);
      inputs.appendChild(numberInput);
      row.appendChild(inputs);
      group.appendChild(row);
    };

    const createBoolRow = () => {
      const row = document.createElement("label");
      row.className = "og-control-row";

      const header = document.createElement("div");
      header.className = "og-control-header";

      const label = document.createElement("span");
      label.className = "og-control-label";
      label.textContent = "Outline";
      header.appendChild(label);
      row.appendChild(header);

      const boolWrap = document.createElement("div");
      boolWrap.className = "og-control-bool";

      const toggle = document.createElement("input");
      toggle.type = "checkbox";
      toggle.className = "og-toggle";
      toggle.checked = state.outline;
      toggle.setAttribute("aria-label", "Outline");

      const status = document.createElement("span");
      status.className = "og-control-bool-state";
      status.textContent = state.outline ? "Enabled" : "Disabled";

      toggle.addEventListener("change", () => {
        state.outline = toggle.checked;
        status.textContent = state.outline ? "Enabled" : "Disabled";
        renderCurrentShape();
      });

      boolWrap.appendChild(toggle);
      boolWrap.appendChild(status);
      row.appendChild(boolWrap);
      panel.appendChild(row);
    };

    const updateGroupVisibility = () => {
      shapeGroups.forEach((group, shape) => {
        group.style.display = shape === state.shape ? "block" : "none";
      });
    };

    (Object.keys(SHAPE_CONTROL_DEFS) as ShapeKind[]).forEach((shape) => {
      const group = document.createElement("div");
      SHAPE_CONTROL_DEFS[shape].forEach((def) => createNumberRow(group, def));
      shapeGroups.set(shape, group);
      panel.appendChild(group);
    });

    createBoolRow();

    const exportButton = document.createElement("button");
    exportButton.type = "button";
    exportButton.textContent = "Export Binary STL";
    exportButton.style.width = "100%";
    exportButton.style.marginTop = "0.25rem";
    exportButton.style.padding = "0.55rem 0.75rem";
    exportButton.style.cursor = "pointer";
    exportButton.style.border = "1px solid #8d8d8d";
    exportButton.style.background = "#f18f33";
    exportButton.style.color = "#171717";
    exportButton.style.fontWeight = "600";
    exportButton.dataset.filename = "opengeometry-cuboid.stl";
    panel.appendChild(exportButton);

    const reportNode = document.createElement("pre");
    reportNode.style.margin = "0.75rem 0 0";
    reportNode.style.fontSize = "0.75rem";
    reportNode.style.lineHeight = "1.45";
    reportNode.style.whiteSpace = "pre-wrap";
    reportNode.textContent = "Generate geometry to enable STL export.";
    panel.appendChild(reportNode);

    document.body.appendChild(panel);

    shapeSelect.addEventListener("change", () => {
      state.shape = shapeSelect.value as ShapeKind;
      updateGroupVisibility();
      renderCurrentShape();
    });

    exportButton.addEventListener("click", () => {
      try {
        const config = {
          header: "OpenGeometry STL Export",
          scale: 1.0,
          error_policy: "BestEffort",
          validate_topology: true,
        };

        const result = manager.exportSceneToStl(
          sceneId,
          JSON.stringify(config)
        );
        const bytes = result.bytes;
        const report = JSON.parse(result.reportJson) as StlExportReport;
        const filename = exportButton.dataset.filename ?? "opengeometry-shape.stl";

        downloadStl(bytes, filename);
        reportNode.textContent = formatReport(report, bytes.length, filename);
      } catch (error) {
        reportNode.textContent = `Export failed: ${String(error)}`;
      }
    });

    updateGroupVisibility();
    renderCurrentShape();
  },
});
