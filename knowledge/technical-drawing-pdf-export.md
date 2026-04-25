# Technical Drawing PDF Export — OpenGeometry + OpenPlans

> **STATUS: IN PROGRESS — PARTIALLY IMPLEMENTED**
>
> **Already in the codebase (as of 2026-04-25):**
> - `OGEntityRegistry` — implemented in `main/opengeometry/src/scenegraph.rs` (alongside `OGSceneManager` which still exists)
> - `EdgeClass` enum — implemented in `main/opengeometry/src/export/projection.rs`
> - `ClassifiedSegment` struct — implemented in `main/opengeometry/src/export/projection.rs`
>
> **Still pending:**
> - The `layouts` TypeScript package under `opengeometry-export-io` (directory exists but is empty — no src/)
> - Sheet composers, PDF/SVG/DXF emitters described in Phases 3–5
> - OpenPlans wiring (Phase 6)
>
> Use this spec for the remaining TypeScript/layouts work. Rust kernel changes are already done.

## Context

OpenGeometry is a Rust/WASM geometry kernel. OpenPlans builds semantic AEC objects (walls, doors, windows) on top of it. Neither currently exports industry-standard technical drawings — the multi-viewport sheets with plan/elevation/section/isometric views used in architectural and engineering workflows.

The goal is end-to-end export of standards-compliant technical drawings (ISO 128, ISO 7200, AIA/NCS) as PDF, SVG (browser preview), and DXF (AutoCAD interchange). Scope matches how AutoCAD paper-space + VIEWBASE works: a sheet document contains multiple viewports, each is a camera into the 3D model at a chosen scale, with classified line weights/types per ISO 128.

**Base branch**: `beta2.0-views-and-layouts` in OpenPlans. Feature branch: `claude/add-pdf-export`.

---

## Architectural Decisions

### Repo responsibilities — strict boundary

**OpenGeometry repo** = geometry kernel only. Rust/WASM + thin TypeScript WASM bindings. No awareness of AEC objects, sheets, paper, or UI frameworks. Changes here are strictly kernel-level.

**OpenPlans repo** = everything application-layer. Semantic objects (wall/door/window), Three.js rendering, sheet/paper model, PDF/SVG/DXF export. No new packages in the OpenGeometry repo.

```
OpenGeometry repo (kernel)        OpenPlans repo (application)
──────────────────────────        ────────────────────────────
Rust/WASM BRep kernel             Elements as Three.js objects (unchanged)
HLR + edge classification         openplans-three — rendering
OGEntityRegistry (WASM)           layouts — Sheet model + PDF/SVG/DXF export
Batched projection API            openplans-core — thin wrapper (future: rename/remove)
Native PDF/DXF/STL (CLI)
```

### `openplans-core` — thin wrapper, do not grow it

`openplans-core` is already a thin wrapper and should stay that way. **Do not add Sheet logic, layout models, or emitters into it.** The right home for those is a new dedicated `layouts` package inside the OpenPlans repo. If `openplans-core` ends up with no meaningful role after this work, rename or remove it with a breaking release.

**Elements stay as Three.js objects.** The current pattern where Wall, Door, Window extend Three.js classes is ergonomic and correct for the existing use case. Do not split elements into data objects + factory pattern — that is over-engineering for a problem (R3F support) that is not in scope for this work. If R3F support becomes a real goal, it is a separate future project with its own breaking changes.

### New `layouts` package in OpenPlans

Sheet model, viewport layout, paper templates, and all export logic (PDF/SVG/DXF) lives in a new `src/packages/layouts/` package. It has:
- No Three.js dependency
- Depends on `@opengeometry/kernel-three` for `CameraParameters`, `HlrOptions`, `OGEntityRegistry`
- Usable from Node.js, plain TypeScript, or any bundler without pulling Three.js into the tree

### OGSceneManager → OGEntityRegistry

The kernel-side registry pattern is **correct and necessary**:
1. WASM memory is separate from JS heap — holding BRep data in WASM avoids re-serialization per viewport per export
2. Batch projection (all walls + doors at once for a sheet) requires the kernel to hold the full entity set

**What to change:**
- Rename `OGSceneManager` → `OGEntityRegistry` — "scene" is a Three.js term; this is a geometry registry
- Add `kind` to registered entities — required for AIA layer assignment (`wall → A-WALL`, `door → A-DOOR`)
- Remove the "currentScene" concept — fragile; replace with explicit entity lists

```typescript
class OGEntityRegistry {
  register(id: string, kind: OGEntityKind, brep: OGBrep): void
  unregister(id: string): void
  projectToViews(views: ViewRequest[]): Record<string, Scene2D>  // batched
  clear(): void
}

type OGEntityKind = "wall" | "door" | "window" | "slab" | "stair" | "column" | "generic"
```

### Architecture split

```
OpenGeometry repo (Rust/WASM)       OpenPlans repo (TypeScript)
──────────────────────────────      ────────────────────────────────────────────
BRep topology                        openplans-three (unchanged Three.js elements)
HLR algorithm                        ├── Wall, Door, Window (extend Object3D)
Edge classification                  ├── PaperFrame Three.js mesh
OGEntityRegistry                     ├── ViewportBlock Three.js mesh
Scene2D (per view)  ──JSON──▶       └── view-render-service, sheet-composer
                                     layouts (NEW — zero Three.js dep)
                                     ├── Sheet / SheetViewport / TitleBlock types
                                     ├── SheetExporter (orchestration)
                                     ├── SheetComposer (pure 2D layout fn)
                                     ├── PdfEmitter  → Blob  (pdf-lib)
                                     ├── SvgEmitter  → string (preview)
                                     └── DxfEmitter  → string (dxf-writer)
```

**Native/CLI path**: keep `pdf.rs` in OpenGeometry for batch/server use. Feed it `ClassifiedSegment` for proper ISO 128 line weights.

---

## Industry Standards Reference

| Standard | Applies To |
|---|---|
| ISO 128-2:2020 | Line types (continuous, dashed, chain) |
| ISO 128-3:2022 | Views, sections, cuts |
| ISO 5455 | Drawing scales: 1:1, 1:5, 1:10, 1:20, 1:50, 1:100, 1:200 |
| ISO 7200 | Title block layout and required fields |
| ASME Y14.3 | Third-angle projection (US) |
| ISO / first-angle | First-angle projection (EU) |
| AIA/NCS | Layer naming: A-WALL, A-DOOR, A-GLAZ, A-FLOR, A-FLOR-STRS |

**Line weights (ISO 128)**: 0.13 / 0.18 / 0.25 / 0.35 / 0.50 / 0.70 / 1.00 / 1.40 / 2.00 mm

**Projection conventions**: First-angle (ISO/Europe) and Third-angle (ANSI/US) — **both supported, user-selectable at sheet creation. No hardcoded default.**

**Edge classification → line style mapping**:

| EdgeClass | Line type | Weight |
|---|---|---|
| VisibleOutline (silhouette) | Continuous | 0.50 mm |
| VisibleCrease (hard edge) | Continuous | 0.25 mm |
| Hidden | Dashed thin | 0.18 mm |
| SectionCut | Chain thick (long-dash-short-dash) | 0.70 mm |
| Centerline | Chain thin (same pattern as SectionCut, thinner weight) | 0.18 mm |
| Dimension | Continuous thin | 0.18 mm |

---

## Critical Files

### OpenGeometry kernel (Rust)

| File | Current State | Action |
|---|---|---|
| `main/opengeometry/src/export/projection.rs` | HLR + Scene2D, bool visible/hidden | **Extend**: add `EdgeClass`, `ClassifiedSegment`, multi-view WASM API |
| `main/opengeometry/src/scenegraph.rs` | `OGSceneManager` + `projectCurrentTo2DLines` | **Rename + extend**: `OGEntityRegistry`, `kind` field, `projectCurrentToViews` |
| `main/opengeometry/src/export/pdf.rs` | Native-only, single view | **Extend**: consume `ClassifiedSegment` for per-class line weights |
| `main/opengeometry/src/export/mod.rs` | Export module | **Extend**: re-export new types |
| `main/opengeometry/Cargo.toml` | `printpdf 0.5`, `dxf 0.6.0` already present | No change needed |

### OpenPlans — `openplans-three` (elements and rendering, largely unchanged)

| File | Action |
|---|---|
| `src/packages/openplans-three/src/sheet-composer.ts` | **Extend**: consume `ClassifiedSegment[]`, apply ISO 128 styles, clip to viewport rect |
| `src/packages/openplans-three/src/view-render-service.ts` | **Extend**: batch all viewports into ONE `projectCurrentToViews` kernel call |
| `src/packages/openplans-three/src/render-registry.ts` | **Extend**: update to call `registerEntity(id, kind, brep)` on `OGEntityRegistry` |
| `src/packages/openplans-core/src/exporter/PlanPDFGenerator.ts` | **Deprecate**: mark `@deprecated`, point callers to `SheetExporter` in `layouts` |

### OpenPlans — `layouts` package (new, `src/packages/layouts/src/`)

| File | Purpose |
|---|---|
| `sheet.ts` | `Sheet`, `SheetViewport`, `TitleBlock`, `Revision`, `SheetMeta` interfaces. No Three.js. JSON-serializable. |
| `sheet-templates.ts` | Paper sizes (ISO A0-A4, ANSI A-E, ARCH A-E) in mm; scale presets; first/third-angle layout algorithms |
| `title-block-iso7200.ts` | ISO 7200 title block — field positions in mm, required fields, border geometry |
| `sheet-exporter.ts` | Orchestration: `Sheet + OGEntityRegistry → project → compose → emit` |
| `sheet-composer.ts` | Pure fn: `compose(sheet, projectedScenes) → SheetComposition`. No renderer dep. Fully testable. |
| `pdf-emitter.ts` | `SheetComposition → Blob` via `pdf-lib` + `@pdf-lib/fontkit`. OCG layers per AIA code. |
| `svg-emitter.ts` | `SheetComposition → SVG string`. Zero deps. Browser preview + snapshot tests. |
| `dxf-emitter.ts` | `SheetComposition → DXF string` via `dxf-writer`. AIA LAYER table + LTYPE table. |
| `line-styles.ts` | ISO 128 default `LineWeightMap`, `LineTypeMap`, dash-pattern definitions |
| `layer-standards.ts` | AIA/NCS mapping: `OGEntityKind → layer code` (wall→A-WALL, door→A-DOOR, window→A-GLAZ, slab→A-FLOR) |
| `dimension-2d.ts` | Pure 2D dimension renderer: sheet-mm points → line + witness lines + arrowheads + text primitives |
| `camera-presets.ts` | Camera factories: `planCamera(bbox, cutHeight)`, `elevationCamera(bbox, dir)`, `isoCamera(bbox)`, `sectionCamera(plane)` |

---

## Data Model

### Kernel: extend `projection.rs` and `scenegraph.rs`

```rust
// New — replaces bool visible/hidden
pub enum EdgeClass {
    VisibleOutline,   // silhouette: front-face edge adjacent to back face
    VisibleCrease,    // sharp angle between two front faces (cos < 0.9995 threshold)
    VisibleSmooth,    // smooth shared edge between front faces (opt-in)
    Hidden,           // behind front-facing geometry
    SectionCut,       // intersects section plane
}

// New — replaces Path2D as output unit
pub struct ClassifiedSegment {
    pub geometry: Segment2D,
    pub class: EdgeClass,
    pub layer: Option<String>,             // AIA code derived from registered entity kind
    pub source_entity_id: Option<String>,
}

// Extend Segment2D — cylinders/spheres/arcs project cleanly instead of as polygon soup
pub enum Segment2D {
    Line { start: Vec2, end: Vec2 },
    Arc { center: Vec2, radius: f64, start_angle: f64, end_angle: f64 },
    Ellipse { center: Vec2, rx: f64, ry: f64, rotation: f64, start_angle: f64, end_angle: f64 },
    CubicBezier { p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2 },
}

// Updated Scene2D — segments replace paths
pub struct Scene2D {
    pub name: Option<String>,
    pub segments: Vec<ClassifiedSegment>,
    pub bounds: Option<(Vec2, Vec2)>,
}

// OGEntityRegistry (renamed from OGSceneManager) — kind field is the key addition
pub struct RegisteredEntity {
    pub id: String,
    pub kind: String,  // "wall" | "door" | "window" | "slab" | "stair" | "generic"
    pub brep: Brep,
}
```

New WASM API in `scenegraph.rs`:
```rust
// Replaces: addBrepEntityToCurrentScene
#[wasm_bindgen(js_name = registerEntity)]
pub fn register_entity(&mut self, id: &str, kind: &str, brep_json: &str) -> Result<(), JsValue>

// New: batched multi-view projection (replaces single-view projectCurrentTo2DLines)
// Input:  JSON array of ViewRequest { id, camera: CameraParameters, hlr: HlrOptions }
// Output: JSON map of { viewportId: Scene2D }
#[wasm_bindgen(js_name = projectCurrentToViews)]
pub fn project_current_to_views(&self, views_json: &str) -> Result<String, JsValue>
```

### TypeScript: `Sheet` types in `layouts` package

```typescript
type ProjectionConvention = "FirstAngle" | "ThirdAngle" | "Custom";
type SheetFormat = "A4"|"A3"|"A2"|"A1"|"A0"
                 | "ANSI_A"|"ANSI_B"|"ANSI_C"|"ANSI_D"|"ANSI_E"
                 | "ARCH_A"|"ARCH_B"|"ARCH_C"|"ARCH_D"|"ARCH_E"
                 | "Custom";
type ViewKind = "Plan"|"ElevationN"|"ElevationS"|"ElevationE"|"ElevationW"
              | "Section"|"Detail"|"Isometric"|"Custom";

// ALL sheet measurements in mm. Model space is meters (kernel convention).
// Scale 1:50 → 1 mm paper = 50 mm model = 0.05 m kernel.

interface Sheet {
  id: string;
  format: SheetFormat;
  orientation: "portrait" | "landscape";
  sizeMm: { width: number; height: number };
  marginMm: number;
  convention: ProjectionConvention;
  viewports: SheetViewport[];
  titleBlock?: TitleBlock;
  annotations: Annotation[];
  revisions: Revision[];
  meta: SheetMeta;
}

interface SheetMeta {
  projectName: string;
  sheetTitle: string;
  drawnBy: string;
  checkedBy: string;
  date: string;
  scale: string;
  sheetNumber: string;
  revisionNumber: string;
  [key: string]: string;
}

interface SheetViewport {
  id: string;
  label: string;                             // "FLOOR PLAN 1:50", "NORTH ELEVATION"
  rectMm: { x: number; y: number; w: number; h: number };
  kind: ViewKind;
  camera: CameraParameters;                  // kernel type, imported from kernel-three
  hlr: HlrOptions;
  scale: { numerator: 1; denominator: number } | { custom: number };
  lineWeights: LineWeightMap;                // EdgeClass → mm
  lineTypes: LineTypeMap;                    // EdgeClass → dash pattern
  layerVisibility: Record<string, boolean>;
  sectionPlane?: { origin: Point3D; normal: Point3D };
  cutHeight?: number;
  dimensions: SheetDimension[];
  clipToRect: boolean;
  frame: { visible: boolean };
}

interface TitleBlock {
  template: "ISO-7200" | "Custom";
  placementMm: { x: number; y: number };
  sizeMm: { width: number; height: number };
  fields: Record<string, string>;
}

interface LineWeightMap {
  visibleOutline: number;  // 0.50
  visibleCrease: number;   // 0.25
  hidden: number;          // 0.18
  sectionCut: number;      // 0.70
  centerline: number;      // 0.18
  dimension: number;       // 0.18
}
```

---

## End-to-End Flow

```
1. Build model (existing API, unchanged)
   // Elements register into OGEntityRegistry inside openplans-three
   const wall = new Wall({ ... })      // Wall extends Three.js Object3D
   scene.add(wall)                     // also calls registry.register("wall-1", "wall", brep)

2. Create sheet (layouts package)
   import { createSheet } from '@opengeometry/layouts'
   const sheet = createSheet({
     format: "A3", orientation: "landscape",
     convention: "ThirdAngle",
     meta: { projectName: "My Project", sheetNumber: "A-101", ... }
   })

3. Add standard views
   addStandardViews(sheet, { registry, scale: 1/50 })
   → camera-presets.ts computes cameras from entity bounding boxes
   → Third-angle: Plan above Front, Right-elev to right of Front
   → First-angle: Plan below Front, Right-elev to left of Front

4. Add section cut (optional)
   addSectionViewport(sheet, {
     sectionPlane: { origin, normal },
     scale: 1/20, label: "SECTION A-A"
   })

5. Export
   import { SheetExporter } from '@opengeometry/layouts'
   const result = await SheetExporter.export(sheet, registry, ["pdf", "svg", "dxf"])

6. SheetExporter internals
   a. Collect ViewRequest[] from sheet.viewports
   b. ONE kernel call: registry.projectCurrentToViews(views_json)
      → returns { viewportId: Scene2D } for ALL views simultaneously
   c. SheetComposer.compose(sheet, projectedScenes)
      → per viewport: scale transform, EdgeClass→style, 2D rect clip, dimensions
      → title block: ISO 7200 field text + border geometry
      → output: SheetComposition (flat styled 2D primitives in mm — no renderer dep)
   d. Emitters via Promise.all:
      PdfEmitter.emit(composition) → Blob
      SvgEmitter.emit(composition) → string
      DxfEmitter.emit(composition) → string

7. Consumer receives ExportResult { pdf?: Blob, svg?: string, dxf?: string }
```

---

## Implementation Phases (Sequenced)

### Phase 1 — Kernel: Edge Classification
**Repo**: OpenGeometry | **Files**: `projection.rs`, `export/mod.rs`

- Add `EdgeClass` enum
- Replace `is_edge_visible() -> bool` with `classify_edge() -> Option<EdgeClass>`
- Emit `ClassifiedSegment` from `project_brep_to_scene`
- Backward compat: `Scene2D::to_lines()` widens `Line2D` with `class: Option<String>`
- Add `Segment2D::Arc` + `Segment2D::Ellipse` variants (cylinders/arcs project as curves)
- Update native `pdf.rs` to apply per-class ISO 128 line weights

### Phase 2 — Kernel: Registry Rename + Multi-View API + Section Slicing
**Repo**: OpenGeometry | **Files**: `scenegraph.rs`, `projection.rs`

- Rename `OGSceneManager` → `OGEntityRegistry`; add `kind: String` to registered entities
- Add `registerEntity(id, kind, brep_json)` replacing `addBrepEntityToCurrentScene`
- Add `projectCurrentToViews(views_json) -> JSON` — batched, one WASM call per export
- Add `sectionPlane: Option<Plane3D>` to projection input
- When section plane is set: intersect BRep faces → emit `SectionCut` segments + closed fill regions
- Add fill region type to `Scene2D` for hatching (poché) downstream

### Phase 3 — `layouts` Package: Data Model + Templates
**Repo**: OpenPlans | **Path**: `src/packages/layouts/src/`
**Files**: `sheet.ts`, `sheet-templates.ts`, `title-block-iso7200.ts`, `line-styles.ts`, `layer-standards.ts`, `camera-presets.ts` (all new)

- All interfaces from data model section
- Paper size constants in mm: ISO A0-A4, ANSI A-E, ARCH A-E
- First-angle and third-angle viewport layout algorithms
- ISO 7200 title block field positions + geometry
- ISO 128 default line weight + type maps
- AIA/NCS entity-kind → layer-code table
- Camera preset factories for standard views

### Phase 4 — `layouts` Package: Composer
**Repo**: OpenPlans | **Files**: `sheet-exporter.ts`, `sheet-composer.ts`, `dimension-2d.ts` (all new)

- `SheetExporter`: collects view requests, calls `OGEntityRegistry.projectCurrentToViews`, calls composer, calls emitters
- `SheetComposer`: pure function — EdgeClass→style, scale transform, 2D rect clipping, dimension geometry, title block rendering
- `dimension-2d.ts`: pure 2D renderer for linear + chain dimensions (no renderer dependency)

### Phase 5 — `layouts` Package: Emitters
**Repo**: OpenPlans | **Files**: `pdf-emitter.ts`, `svg-emitter.ts`, `dxf-emitter.ts` (all new)

**PDF** (`pdf-lib` + `@pdf-lib/fontkit`):
- Page sized to sheet.sizeMm (mm → pt: × 2.8346)
- Vector lines with explicit dash arrays per ISO 128 line type
- OCG layers per AIA code (visible in Acrobat layer panel)
- Embedded + subsetted TTF font (one OFL sans-serif)

**SVG** (zero dependencies):
- `<svg viewBox="0 0 {w} {h}">` in mm units
- `<clipPath>` per viewport, `stroke-dasharray` for line types
- `<g>` layer grouping per AIA code
- Browser preview and vitest snapshot tests

**DXF** (`dxf-writer` npm):
- LAYER table: one entry per AIA code
- LTYPE table: CONTINUOUS, DASHED, CENTER, PHANTOM
- Paper-space LAYOUT + VIEWPORT entities
- MTEXT for dimension values and title block fields

### Phase 6 — Wire OpenPlans-Three to New APIs
**Repo**: OpenPlans | **Files**: `render-registry.ts`, `view-render-service.ts`, `PlanPDFGenerator.ts`

- `render-registry.ts`: call `registerEntity(id, kind, brep)` on `OGEntityRegistry` when elements are created (replaces `addBrepEntityToCurrentScene`)
- `view-render-service.ts`: update to call `OGEntityRegistry.projectCurrentToViews` (new batched API)
- Mark `PlanPDFGenerator` as `@deprecated` with migration note pointing to `layouts/SheetExporter`

---

## Critical Bugs to Fix

### 1. Unit mismatch in `paper-frame.ts` (confirmed bug)
`paperSizes` stores `A4: { width: 21.0, height: 29.7 }` — these are **centimetres**, not mm. ISO A4 = 210 × 297 mm. `createInnerBorder` uses `margin / 10` as a silent cm→unknown conversion.

**Fix** (in `openplans-three`, where PaperFrame renderer lives):
- Change `paperSizes` to `A4: { width: 210, height: 297 }` (mm)
- Define `const THREE_UNITS_PER_MM = 0.1` (1 Three.js unit = 1 cm)
- Replace `margin / 10` with `margin * THREE_UNITS_PER_MM`
- Add a top-of-file comment declaring the unit convention

### 2. `PlanPDFGenerator.ts` produces raster output
Renders WebGL to canvas and embeds JPEG. Not vector, not scalable, not standards-compliant. Mark `@deprecated`, replace with `layouts/SheetExporter`.

### 3. `printpdf` is native-only
`export_scene_to_pdf_bytes` compiles only with `cfg(not(target_arch = "wasm32"))`. Do not use from browser. Browser PDF = `pdf-lib` in the `layouts` package only.

### 4. Centerline and section-cut are the same dash pattern (ISO 128)
Both are long-dash-short-dash (chain family). Cutting-plane = chain thick. Centerline = chain thin. Same `stroke-dasharray`, different `stroke-width`. Do not implement as two different patterns.

---

## New Dependencies

### `layouts` package `package.json`
```json
"pdf-lib": "^1.17.1",
"@pdf-lib/fontkit": "^1.1.1",
"dxf-writer": "^1.6.0"
```

`layouts` has zero Three.js dependency. `openplans-core` gets no new dependencies.

### Font strategy
Bundle one OFL-licensed sans-serif (Inter, IBM Plex Sans, or Source Sans 3). Single `.ttf`, subsetted at emit time via `@pdf-lib/fontkit`. No font handling in Rust/WASM.

---

## Verification

1. **Kernel unit tests**: cube corner edges = `VisibleCrease`, silhouette from angled camera = `VisibleOutline`, rear faces = `Hidden`
2. **Registry kind test**: `registerEntity("w1", "wall", brep)` → `projectCurrentToViews` returns segments with `layer = "A-WALL"`
3. **Multi-view test**: project wall+door to 4 cameras in one call, assert 4 non-empty `Scene2D` results
4. **Section test**: horizontal section plane at cut height → assert `SectionCut` segments only at that height
5. **SheetComposer unit test**: known `Scene2D` + `SheetViewport` → assert output mm coordinates and ISO 128 weights
6. **SVG snapshot test**: `SvgEmitter.emit()` → vitest snapshot comparison
7. **PDF smoke test**: `PdfEmitter.emit()` → assert starts with `%PDF-`, `pdf-lib` can re-parse it
8. **DXF validation**: open in LibreCAD or ODA File Converter — AIA layers and LTYPE entries present
9. **layouts zero-Three.js test**: install `layouts` in a project with no `three` — must compile cleanly
10. **End-to-end**: wall+window+door registered → `SheetExporter.export(sheet, registry, ["pdf","svg","dxf"])` → non-zero `ExportResult`
