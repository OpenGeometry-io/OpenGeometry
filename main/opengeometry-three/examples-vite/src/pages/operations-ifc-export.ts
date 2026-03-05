import { Cuboid, Cylinder, OGSceneManager, Sphere, Vector3, Wedge } from "@og-three";
import * as THREE from "three";
import { bootstrapExample, replaceSceneObject } from "../shared/runtime";

interface IfcExportReport {
  input_breps: number;
  input_faces: number;
  exported_elements: number;
  exported_faces: number;
  exported_triangles: number;
  skipped_entities: number;
  skipped_faces: number;
  topology_errors: number;
  semantics_applied: number;
  proxy_fallbacks: number;
  property_sets_written: number;
  quantity_sets_written: number;
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

const IFC_ENTITY_ID = "ifc-shape";

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function downloadText(text: string, filename: string, mime: string) {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

function defaultSemantics(entityId: string): string {
  const payload = {
    [entityId]: {
      ifc_class: "IFCWALL",
      name: "Sample Wall",
      description: "User-side semantics override from sidecar JSON",
      object_type: "Partition",
      tag: "W-001",
      property_sets: {
        Pset_WallCommon: {
          FireRating: "2h",
          IsExternal: "false",
        },
      },
      quantity_sets: {
        Qto_WallBaseQuantities: {
          Length: 5.0,
        },
      },
    },
  };

  return JSON.stringify(payload, null, 2);
}

async function tryWebIfcParse(ifcText: string): Promise<string> {
  try {
    const moduleUrl =
      "https://cdn.jsdelivr.net/npm/web-ifc@0.0.75/web-ifc-api.js";
    const mod = await import(/* @vite-ignore */ moduleUrl);
    const IfcAPI = (mod as { IfcAPI?: new () => unknown }).IfcAPI;

    if (!IfcAPI) {
      return "web-ifc parser unavailable in runtime module.";
    }

    const api = new (IfcAPI as new () => {
      Init: () => Promise<void>;
      OpenModel: (data: Uint8Array, settings?: Record<string, unknown>) => number;
      GetMaxExpressID: (modelId: number) => number;
      CloseModel: (modelId: number) => void;
    })();

    await api.Init();
    const bytes = new TextEncoder().encode(ifcText);
    const modelId = api.OpenModel(bytes, { COORDINATE_TO_ORIGIN: false });
    const maxExpressId = api.GetMaxExpressID(modelId);
    api.CloseModel(modelId);

    return `web-ifc parse: OK (max express id: ${maxExpressId})`;
  } catch (error) {
    return `web-ifc parse skipped: ${String(error)}`;
  }
}

function formatReport(
  report: IfcExportReport,
  bytes: number,
  filename: string,
  parseMessage: string
): string {
  return [
    `File: ${filename}`,
    `Bytes: ${bytes}`,
    parseMessage,
    `Input BReps: ${report.input_breps}`,
    `Input Faces: ${report.input_faces}`,
    `Exported Elements: ${report.exported_elements}`,
    `Exported Faces: ${report.exported_faces}`,
    `Exported Triangles: ${report.exported_triangles}`,
    `Skipped Entities: ${report.skipped_entities}`,
    `Skipped Faces: ${report.skipped_faces}`,
    `Topology Errors: ${report.topology_errors}`,
    `Semantics Applied: ${report.semantics_applied}`,
    `Proxy Fallbacks: ${report.proxy_fallbacks}`,
    `Property Sets Written: ${report.property_sets_written}`,
    `Quantity Sets Written: ${report.quantity_sets_written}`,
  ].join("\n");
}

bootstrapExample({
  title: "Operation: IFC Export",
  description:
    "IFC4 export with optional user-side semantics sidecar JSON and web-ifc parse probe.",
  build: ({ scene }) => {
    const manager = new OGSceneManager();
    const sceneId = manager.createScene("ifc-export-scene");

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
    heading.textContent = "IFC Export";
    panel.appendChild(heading);

    const summary = document.createElement("p");
    summary.style.margin = "0 0 0.75rem";
    summary.style.fontSize = "0.75rem";
    summary.style.color = "#5f5f5f";
    summary.textContent =
      "Pick a shape, edit semantics JSON keyed by entity id, and export IFC4.";
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
    shapeSelect.style.font =
      '11px/1.2 "IBM Plex Mono", "JetBrains Mono", "SFMono-Regular", monospace';

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
            fileName = "opengeometry-cuboid.ifc";
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
            fileName = "opengeometry-cylinder.ifc";
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
            fileName = "opengeometry-sphere.ifc";
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
            fileName = "opengeometry-wedge.ifc";
            readyText = `Ready: Wedge (W=${width.toFixed(2)}, H=${height.toFixed(2)}, D=${depth.toFixed(2)})`;
            break;
          }
        }

        current = replaceSceneObject(scene, current, object);
        manager.addBrepEntityToScene(sceneId, IFC_ENTITY_ID, kind, JSON.stringify(brepData));

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

    const semanticsRow = document.createElement("div");
    semanticsRow.className = "og-control-row";

    const semanticsHeader = document.createElement("div");
    semanticsHeader.className = "og-control-header";

    const semanticsLabel = document.createElement("span");
    semanticsLabel.className = "og-control-label";
    semanticsLabel.textContent = "IFC Semantics Sidecar";
    semanticsHeader.appendChild(semanticsLabel);
    semanticsRow.appendChild(semanticsHeader);

    const semanticsEditor = document.createElement("textarea");
    semanticsEditor.value = defaultSemantics(IFC_ENTITY_ID);
    semanticsEditor.style.width = "100%";
    semanticsEditor.style.minHeight = "150px";
    semanticsEditor.style.border = "1px solid #b8b8b8";
    semanticsEditor.style.background = "#dfdfdf";
    semanticsEditor.style.color = "#242424";
    semanticsEditor.style.padding = "6px";
    semanticsEditor.style.font =
      '11px/1.3 "IBM Plex Mono", "JetBrains Mono", "SFMono-Regular", monospace';
    semanticsEditor.style.resize = "vertical";
    semanticsRow.appendChild(semanticsEditor);
    panel.appendChild(semanticsRow);

    const exportButton = document.createElement("button");
    exportButton.type = "button";
    exportButton.textContent = "Export IFC";
    exportButton.style.width = "100%";
    exportButton.style.marginTop = "0.25rem";
    exportButton.style.padding = "0.55rem 0.75rem";
    exportButton.style.cursor = "pointer";
    exportButton.style.border = "1px solid #8d8d8d";
    exportButton.style.background = "#f18f33";
    exportButton.style.color = "#171717";
    exportButton.style.fontWeight = "600";
    exportButton.dataset.filename = "opengeometry-cuboid.ifc";
    panel.appendChild(exportButton);

    const reportNode = document.createElement("pre");
    reportNode.style.margin = "0.75rem 0 0";
    reportNode.style.fontSize = "0.75rem";
    reportNode.style.lineHeight = "1.45";
    reportNode.style.whiteSpace = "pre-wrap";
    reportNode.textContent = "Generate geometry to enable IFC export.";
    panel.appendChild(reportNode);

    document.body.appendChild(panel);

    shapeSelect.addEventListener("change", () => {
      state.shape = shapeSelect.value as ShapeKind;
      updateGroupVisibility();
      renderCurrentShape();
    });

    exportButton.addEventListener("click", async () => {
      try {
        let semanticsMap: unknown;
        try {
          semanticsMap = JSON.parse(semanticsEditor.value);
        } catch (error) {
          reportNode.textContent = `Semantics JSON is invalid: ${String(error)}`;
          return;
        }

        const config = {
          schema: "Ifc4Add2",
          project_name: "OpenGeometry IFC Export",
          site_name: "Main Site",
          building_name: "Main Building",
          storey_name: "Level 01",
          scale: 1.0,
          error_policy: "BestEffort",
          validate_topology: true,
          require_closed_shell: true,
          semantics: semanticsMap,
        };

        const result = manager.exportSceneToIfc(sceneId, JSON.stringify(config));
        const report = JSON.parse(result.reportJson) as IfcExportReport;
        const filename = exportButton.dataset.filename ?? "opengeometry-shape.ifc";

        downloadText(result.text, filename, "model/ifc");
        const parseMessage = await tryWebIfcParse(result.text);
        reportNode.textContent = formatReport(
          report,
          new TextEncoder().encode(result.text).length,
          filename,
          parseMessage
        );
      } catch (error) {
        reportNode.textContent = `Export failed: ${String(error)}`;
      }
    });

    updateGroupVisibility();
    renderCurrentShape();
  },
});
