import { Cuboid, OGSceneManager, Polyline, Sweep, Vector3 } from "@og-three";
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

interface StepExportReport {
  input_breps: number;
  input_faces: number;
  exported_solids: number;
  exported_faces: number;
  exported_triangles: number;
  skipped_entities: number;
  skipped_faces: number;
  topology_errors: number;
}

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

type NumberKey =
  | "doorWidth"
  | "doorHeight"
  | "panelThickness"
  | "frameDepth"
  | "frameFace"
  | "frameRebate"
  | "frameClearance";

interface NumberControlDef {
  key: NumberKey;
  label: string;
  min: number;
  max: number;
  step: number;
}

const PANEL_ENTITY_ID = "ifc-door-panel";
const FRAME_ENTITY_ID = "ifc-door-frame";
const DOOR_POSITIONS: Array<{ id: string; x: number; z: number }> = [
  { id: "A", x: -2.2, z: -1.8 },
  { id: "B", x: 2.2, z: -1.8 },
  { id: "C", x: -2.2, z: 1.8 },
  { id: "D", x: 2.2, z: 1.8 },
];
const EXPECTED_ENTITY_COUNT = DOOR_POSITIONS.length * 2;

const NUMBER_CONTROLS: NumberControlDef[] = [
  { key: "doorWidth", label: "Door Width", min: 0.6, max: 2.0, step: 0.01 },
  { key: "doorHeight", label: "Door Height", min: 1.8, max: 3.0, step: 0.01 },
  { key: "panelThickness", label: "Panel Thickness", min: 0.02, max: 0.12, step: 0.005 },
  { key: "frameDepth", label: "Frame Depth", min: 0.06, max: 0.3, step: 0.005 },
  { key: "frameFace", label: "Frame Face", min: 0.02, max: 0.15, step: 0.005 },
  { key: "frameRebate", label: "Frame Rebate", min: 0.01, max: 0.12, step: 0.005 },
  { key: "frameClearance", label: "Panel Clearance", min: 0.002, max: 0.04, step: 0.001 },
];

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function formatNumber(value: number): string {
  return value.toFixed(3).replace(/\.?0+$/, "");
}

function clonePoints(points: Vector3[]): Vector3[] {
  return points.map((point) => point.clone());
}

function closeLoop(points: Vector3[]): Vector3[] {
  if (points.length === 0) {
    return [];
  }
  return [...points, points[0].clone()];
}

function buildFramePath(width: number, height: number, clearance: number): Vector3[] {
  const xLeft = -width * 0.5 - clearance;
  const xRight = width * 0.5 + clearance;
  return [
    new Vector3(xLeft, 0, 0),
    new Vector3(xLeft, height, 0),
    new Vector3(xRight, height, 0),
    new Vector3(xRight, 0, 0),
  ];
}

function buildFrameProfile(
  frameDepth: number,
  frameFace: number,
  frameRebate: number
): Vector3[] {
  const depth = Math.max(frameDepth, frameFace + 0.004, frameRebate + 0.004);
  const face = clamp(frameFace, 0.002, depth - 0.002);
  const rebate = clamp(frameRebate, 0.002, depth - 0.002);

  const raw = [
    new Vector3(depth, 0, depth),
    new Vector3(rebate, 0, depth),
    new Vector3(rebate, 0, face),
    new Vector3(0, 0, face),
    new Vector3(0, 0, 0),
    new Vector3(depth, 0, 0),
  ];

  const avgZ = raw.reduce((sum, point) => sum + point.z, 0) / raw.length;
  const orientStart = new Vector3(depth, 0, avgZ);

  return [orientStart, ...raw];
}

function buildProfileGuide(profile: Vector3[], doorWidth: number): Vector3[] {
  const xOffset = -doorWidth * 0.95;
  const yOffset = 0.2;
  const zOffset = 0.18;

  return closeLoop(profile).map(
    (point) => new Vector3(xOffset + point.x, yOffset + point.z, zOffset)
  );
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

function downloadBytes(bytes: Uint8Array, filename: string, mime: string) {
  const blob = new Blob([bytes], { type: mime });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

function formatStlReport(report: StlExportReport, bytes: number, filename: string): string {
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

function formatStepReport(report: StepExportReport, bytes: number, filename: string): string {
  return [
    `File: ${filename}`,
    `Bytes: ${bytes}`,
    `Input BReps: ${report.input_breps}`,
    `Input Faces: ${report.input_faces}`,
    `Exported Solids: ${report.exported_solids}`,
    `Exported Faces: ${report.exported_faces}`,
    `Exported Triangles: ${report.exported_triangles}`,
    `Skipped Entities: ${report.skipped_entities}`,
    `Skipped Faces: ${report.skipped_faces}`,
    `Topology Errors: ${report.topology_errors}`,
  ].join("\n");
}

function formatIfcReport(
  report: IfcExportReport,
  bytes: number,
  filename: string,
  ifcDoorMentions: number,
  isPass: boolean,
  expectedEntities: number
): string {
  return [
    `File: ${filename}`,
    `Bytes: ${bytes}`,
    `IFCDOOR Mentions: ${ifcDoorMentions}`,
    `Expected IFCDOOR Entities: ${expectedEntities}`,
    `IFC Criteria: ${isPass ? "PASS" : "FAIL"}`,
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

function buildIfcSemantics(
  doorWidth: number,
  doorHeight: number,
  panelWidth: number,
  panelHeight: number
): Record<string, unknown> {
  const semantics: Record<string, unknown> = {};

  DOOR_POSITIONS.forEach((door, index) => {
    const suffix = String(index + 1).padStart(2, "0");
    const panelId = `${PANEL_ENTITY_ID}-${door.id}`;
    const frameId = `${FRAME_ENTITY_ID}-${door.id}`;

    semantics[panelId] = {
      ifc_class: "IFCDOOR",
      name: `Door ${suffix} Panel`,
      object_type: "DoorLeaf",
      tag: `D-${suffix}-PANEL`,
      property_sets: {
        Pset_DoorCommon: {
          FireRating: "30min",
          IsExternal: "false",
        },
      },
      quantity_sets: {
        Qto_DoorBaseQuantities: {
          Width: Number(panelWidth.toFixed(4)),
          Height: Number(panelHeight.toFixed(4)),
        },
      },
    };

    semantics[frameId] = {
      ifc_class: "IFCDOOR",
      name: `Door ${suffix} Frame`,
      object_type: "DoorFrame",
      tag: `D-${suffix}-FRAME`,
      property_sets: {
        Pset_DoorCommon: {
          FireRating: "30min",
          IsExternal: "false",
        },
      },
      quantity_sets: {
        Qto_DoorBaseQuantities: {
          Width: Number(doorWidth.toFixed(4)),
          Height: Number(doorHeight.toFixed(4)),
        },
      },
    };
  });

  return semantics;
}

bootstrapExample({
  title: "Operation: Door Export",
  description:
    "Door panel as Cuboid + frame from swept custom profile, exported via STL/STEP/IFC.",
  build: ({ scene }) => {
    const manager = new OGSceneManager();
    const sceneId = manager.createScene("door-export-scene");
    let current: THREE.Group | null = null;

    const state: {
      outline: boolean;
      showFrame: boolean;
      showPanel: boolean;
      values: Record<NumberKey, number>;
    } = {
      outline: true,
      showFrame: true,
      showPanel: true,
      values: {
        doorWidth: 0.95,
        doorHeight: 2.1,
        panelThickness: 0.04,
        frameDepth: 0.12,
        frameFace: 0.05,
        frameRebate: 0.025,
        frameClearance: 0.012,
      },
    };

    const panel = document.createElement("aside");
    panel.className = "og-controls";

    const heading = document.createElement("h3");
    heading.textContent = "Door Export";
    panel.appendChild(heading);

    const summary = document.createElement("p");
    summary.style.margin = "0 0 0.75rem";
    summary.style.fontSize = "0.75rem";
    summary.style.color = "#5f5f5f";
    summary.textContent =
      "Frame is a custom profile sweep. Door panel is a Cuboid. 4 doors are placed and exported together.";
    panel.appendChild(summary);

    const reportNode = document.createElement("pre");
    reportNode.style.margin = "0.75rem 0 0";
    reportNode.style.fontSize = "0.75rem";
    reportNode.style.lineHeight = "1.45";
    reportNode.style.whiteSpace = "pre-wrap";

    const renderDoorAssembly = () => {
      try {
        const doorWidth = state.values.doorWidth;
        const doorHeight = state.values.doorHeight;
        const panelThickness = state.values.panelThickness;
        const frameClearance = state.values.frameClearance;

        const frameDepth = Math.max(
          state.values.frameDepth,
          state.values.frameFace + 0.004,
          state.values.frameRebate + 0.004
        );
        const frameFace = clamp(state.values.frameFace, 0.002, frameDepth - 0.002);
        const frameRebate = clamp(state.values.frameRebate, 0.002, frameDepth - 0.002);

        const framePath = buildFramePath(doorWidth, doorHeight, frameClearance);
        const frameProfile = buildFrameProfile(frameDepth, frameFace, frameRebate);

        const panelWidth = Math.max(0.1, doorWidth - frameClearance * 2);
        const panelHeight = Math.max(0.2, doorHeight - frameClearance * 2);
        const panelCenterY = panelHeight * 0.5 + frameClearance;

        const frameProfileGuide = new Polyline({
          points: buildProfileGuide(frameProfile, doorWidth),
          color: 0xf97316,
        });

        const group = new THREE.Group();
        if (state.showFrame) {
          group.add(frameProfileGuide);
        }

        DOOR_POSITIONS.forEach((door) => {
          const framePathAtDoor = framePath.map(
            (point) => new Vector3(point.x + door.x, point.y, point.z + door.z)
          );

          const frame = new Sweep({
            path: clonePoints(framePathAtDoor),
            profile: clonePoints(frameProfile),
            color: 0x64748b,
            capStart: true,
            capEnd: true,
          });
          frame.outline = state.outline;

          const doorPanel = new Cuboid({
            center: new Vector3(door.x, panelCenterY, door.z),
            width: panelWidth,
            height: panelHeight,
            depth: panelThickness,
            color: 0x2563eb,
          });
          doorPanel.outline = state.outline;

          const framePathGuide = new Polyline({
            points: clonePoints(framePathAtDoor),
            color: 0x1f2937,
          });
          framePathGuide.position.z += frameDepth * 0.6;

          if (state.showFrame) {
            group.add(frame);
            group.add(framePathGuide);
          }
          if (state.showPanel) {
            group.add(doorPanel);
          }

          manager.addBrepEntityToScene(
            sceneId,
            `${FRAME_ENTITY_ID}-${door.id}`,
            "OGSweep",
            JSON.stringify(frame.getBrep())
          );
          manager.addBrepEntityToScene(
            sceneId,
            `${PANEL_ENTITY_ID}-${door.id}`,
            "OGCuboid",
            JSON.stringify(doorPanel.getBrepData())
          );
        });

        current = replaceSceneObject(scene, current, group);

        reportNode.textContent = [
          `Ready: ${DOOR_POSITIONS.length} Door Assemblies (W=${formatNumber(doorWidth)}, H=${formatNumber(doorHeight)})`,
          `Panel: T=${formatNumber(panelThickness)} W=${formatNumber(panelWidth)} H=${formatNumber(panelHeight)}`,
          `Frame Profile: Depth=${formatNumber(frameDepth)} Face=${formatNumber(frameFace)} Rebate=${formatNumber(frameRebate)}`,
          `Visible: Frame=${state.showFrame ? "On" : "Off"} Panel=${state.showPanel ? "On" : "Off"}`,
          `Export Entities: ${EXPECTED_ENTITY_COUNT}`,
        ].join("\n");
      } catch (error) {
        reportNode.textContent = `Door assembly update failed: ${String(error)}`;
      }
    };

    const createNumberRow = (def: NumberControlDef) => {
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
        valueLabel.textContent = formatNumber(value);
      };

      const updateValue = (raw: number) => {
        const next = clamp(raw, def.min, def.max);
        syncDisplay(next);
        renderDoorAssembly();
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
      panel.appendChild(row);
    };

    const createBoolRow = (
      labelText: string,
      checked: boolean,
      onToggle: (next: boolean) => void
    ) => {
      const row = document.createElement("label");
      row.className = "og-control-row";

      const header = document.createElement("div");
      header.className = "og-control-header";

      const label = document.createElement("span");
      label.className = "og-control-label";
      label.textContent = labelText;
      header.appendChild(label);
      row.appendChild(header);

      const boolWrap = document.createElement("div");
      boolWrap.className = "og-control-bool";

      const toggle = document.createElement("input");
      toggle.type = "checkbox";
      toggle.className = "og-toggle";
      toggle.checked = checked;
      toggle.setAttribute("aria-label", labelText);

      const status = document.createElement("span");
      status.className = "og-control-bool-state";
      status.textContent = checked ? "Enabled" : "Disabled";

      toggle.addEventListener("change", () => {
        onToggle(toggle.checked);
        status.textContent = toggle.checked ? "Enabled" : "Disabled";
        renderDoorAssembly();
      });

      boolWrap.appendChild(toggle);
      boolWrap.appendChild(status);
      row.appendChild(boolWrap);
      panel.appendChild(row);
    };

    NUMBER_CONTROLS.forEach((def) => createNumberRow(def));
    createBoolRow("Show Frame", state.showFrame, (next) => {
      state.showFrame = next;
    });
    createBoolRow("Show Panel", state.showPanel, (next) => {
      state.showPanel = next;
    });
    createBoolRow("Outline", state.outline, (next) => {
      state.outline = next;
    });

    const buttonStyles = {
      width: "100%",
      marginTop: "0.35rem",
      padding: "0.55rem 0.75rem",
      cursor: "pointer",
      border: "1px solid #8d8d8d",
      color: "#171717",
      fontWeight: "600",
    } as const;

    const stlButton = document.createElement("button");
    stlButton.type = "button";
    stlButton.textContent = "Export Binary STL";
    stlButton.style.width = buttonStyles.width;
    stlButton.style.marginTop = buttonStyles.marginTop;
    stlButton.style.padding = buttonStyles.padding;
    stlButton.style.cursor = buttonStyles.cursor;
    stlButton.style.border = buttonStyles.border;
    stlButton.style.background = "#22d3ee";
    stlButton.style.color = buttonStyles.color;
    stlButton.style.fontWeight = buttonStyles.fontWeight;

    const stepButton = document.createElement("button");
    stepButton.type = "button";
    stepButton.textContent = "Export STEP";
    stepButton.style.width = buttonStyles.width;
    stepButton.style.marginTop = buttonStyles.marginTop;
    stepButton.style.padding = buttonStyles.padding;
    stepButton.style.cursor = buttonStyles.cursor;
    stepButton.style.border = buttonStyles.border;
    stepButton.style.background = "#f59e0b";
    stepButton.style.color = buttonStyles.color;
    stepButton.style.fontWeight = buttonStyles.fontWeight;

    const ifcButton = document.createElement("button");
    ifcButton.type = "button";
    ifcButton.textContent = "Export IFC Door";
    ifcButton.style.width = buttonStyles.width;
    ifcButton.style.marginTop = buttonStyles.marginTop;
    ifcButton.style.padding = buttonStyles.padding;
    ifcButton.style.cursor = buttonStyles.cursor;
    ifcButton.style.border = buttonStyles.border;
    ifcButton.style.background = "#fb7185";
    ifcButton.style.color = buttonStyles.color;
    ifcButton.style.fontWeight = buttonStyles.fontWeight;

    stlButton.addEventListener("click", () => {
      try {
        const config = {
          header: "OpenGeometry Door Assembly STL Export",
          scale: 1.0,
          error_policy: "BestEffort",
          validate_topology: true,
        };

        const result = manager.exportSceneToStl(sceneId, JSON.stringify(config));
        const report = JSON.parse(result.reportJson) as StlExportReport;
        const filename = "opengeometry-door-assembly.stl";
        downloadBytes(result.bytes, filename, "model/stl");
        reportNode.textContent = formatStlReport(report, result.bytes.length, filename);
      } catch (error) {
        reportNode.textContent = `STL export failed: ${String(error)}`;
      }
    });

    stepButton.addEventListener("click", () => {
      try {
        const config = {
          schema: "AutomotiveDesign",
          product_name: "OpenGeometry Door Assembly STEP Export",
          scale: 1.0,
          error_policy: "BestEffort",
          validate_topology: true,
          require_closed_shell: true,
        };

        const result = manager.exportSceneToStep(sceneId, JSON.stringify(config));
        const report = JSON.parse(result.reportJson) as StepExportReport;
        const filename = "opengeometry-door-assembly.step";
        downloadText(result.text, filename, "application/step");
        reportNode.textContent = formatStepReport(
          report,
          new TextEncoder().encode(result.text).length,
          filename
        );
      } catch (error) {
        reportNode.textContent = `STEP export failed: ${String(error)}`;
      }
    });

    ifcButton.addEventListener("click", () => {
      try {
        const doorWidth = state.values.doorWidth;
        const doorHeight = state.values.doorHeight;
        const frameClearance = state.values.frameClearance;
        const panelWidth = Math.max(0.1, doorWidth - frameClearance * 2);
        const panelHeight = Math.max(0.2, doorHeight - frameClearance * 2);

        const config = {
          schema: "Ifc4Add2",
          project_name: "OpenGeometry Door Export",
          site_name: "Main Site",
          building_name: "Main Building",
          storey_name: "Level 01",
          scale: 1.0,
          error_policy: "BestEffort",
          validate_topology: true,
          require_closed_shell: true,
          semantics: buildIfcSemantics(doorWidth, doorHeight, panelWidth, panelHeight),
        };

        const result = manager.exportSceneToIfc(sceneId, JSON.stringify(config));
        const report = JSON.parse(result.reportJson) as IfcExportReport;
        const filename = "opengeometry-door-assembly.ifc";
        const bytes = new TextEncoder().encode(result.text).length;
        const ifcDoorMentions = (result.text.match(/IFCDOOR\(/g) ?? []).length;
        const isPass =
          report.exported_elements >= EXPECTED_ENTITY_COUNT &&
          report.semantics_applied >= EXPECTED_ENTITY_COUNT &&
          report.proxy_fallbacks === 0 &&
          ifcDoorMentions >= EXPECTED_ENTITY_COUNT;

        downloadText(result.text, filename, "model/ifc");
        reportNode.textContent = formatIfcReport(
          report,
          bytes,
          filename,
          ifcDoorMentions,
          isPass,
          EXPECTED_ENTITY_COUNT
        );
      } catch (error) {
        reportNode.textContent = `IFC export failed: ${String(error)}`;
      }
    });

    panel.appendChild(stlButton);
    panel.appendChild(stepButton);
    panel.appendChild(ifcButton);

    reportNode.textContent = "Generate geometry to enable door assembly exports.";
    panel.appendChild(reportNode);

    document.body.appendChild(panel);
    renderDoorAssembly();
  },
});
