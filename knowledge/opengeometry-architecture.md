# OpenGeometry Architecture and Engine Drivers

This note documents how the repository is structured and what actually drives geometry generation, scene composition, and projection/export.

## 1) Repository Structure

```text
OpenGeometry/
├── main/
│   ├── opengeometry/                # Rust core (WASM + native)
│   │   ├── src/
│   │   │   ├── brep/                # Core topology data model (Vertex/Edge/Face/Brep)
│   │   │   ├── primitives/          # OGLine, OGPolyline, OGPolygon, OGCuboid, OGCylinder, ...
│   │   │   ├── operations/          # triangulate, extrude, winding utilities
│   │   │   ├── export/              # 2D projection + optional PDF export
│   │   │   └── scenegraph.rs        # OGSceneManager orchestration layer
│   │   └── examples/                # Projection/PDF usage samples
│   ├── opengeometry-three/          # TS/Three.js wrapper around WASM classes
│   │   ├── src/primitives/          # Three wrappers for line/arc/polyline/rectangle
│   │   └── src/shapes/              # Three wrappers for polygon/cuboid/cylinder/opening
│   ├── opengeometry-webgl/          # Package scaffold
│   └── opengeometry-babylon/        # Package scaffold
├── dist/                            # Final JS bundle + wasm binary + d.ts
├── developer.md                     # Existing hand-maintained project notes
└── knowledge.md                     # Existing domain notes
```

## 2) Component Architecture

```mermaid
flowchart LR
    app["App Code<br/>Browser / Node"] --> wrapper["kernel-three wrapper<br/>TypeScript adapter"]
    wrapper --> glue["wasm-bindgen JS glue<br/>main/opengeometry/pkg"]
    glue --> wasm["opengeometry_bg.wasm<br/>Rust kernel"]

    subgraph kernel["Rust kernel modules"]
      wasm --> primitives["primitives/*<br/>construct BRep"]
      wasm --> operations["operations/*<br/>triangulate + extrude + winding"]
      wasm --> scenegraph["scenegraph.rs<br/>OGSceneManager"]
      wasm --> projection["export/projection.rs<br/>Camera + HLR -> Scene2D"]
      wasm --> pdf["export/pdf.rs<br/>native PDF only"]
    end

    primitives --> brep["brep/*<br/>Vertex Edge Face BRep"]
    operations --> brep
    scenegraph --> brep
    projection --> brep
    scenegraph --> projection
    projection --> scene2d["Scene2D / Scene2DLines JSON"]
    pdf --> pdfout["PDF file output"]
```

## 3) Runtime Flow (What Drives the Engine)

```mermaid
sequenceDiagram
    participant U as User/App
    participant T as opengeometry-three
    participant W as WASM (Rust primitive)
    participant S as OGSceneManager
    participant P as Projection/PDF

    U->>T: OpenGeometry.create()
    T->>W: init(wasmURL)
    U->>T: new Polygon/Cuboid/Line(...), setConfig(...)
    T->>W: set_config(...)
    W->>W: build/update Brep (vertices/edges/faces)
    U->>T: generate/render in Three
    T->>W: get_geometry_serialized()
    W-->>T: Float buffer JSON
    T->>T: Build THREE.BufferGeometry

    U->>S: add*ToScene(...), projectTo2D*(camera, hlr)
    S->>P: project_brep_to_scene(entity.brep, camera, hlr)
    P-->>S: Scene2D / Scene2DLines
    S-->>U: JSON payload (or PDF in native build)
```

### Core engine drivers

1. `Brep` is the canonical geometry/topology state.
2. Primitive `set_config` + generation methods populate `Brep`.
3. `triangulate_polygon_with_holes` (Earcut-based) drives mesh buffers for filled faces.
4. `extrude_brep_face` drives volume creation for cuboid/cylinder-style solids.
5. `OGSceneManager` drives multi-entity orchestration and projection calls.
6. `project_brep_to_scene` drives view projection, near-plane clipping, and optional hidden-line removal (`HlrOptions.hide_hidden_edges`).
7. `export_scene_to_pdf_with_config` drives native vector PDF output from projected 2D paths.

## 4) Build and Packaging Pipeline

```mermaid
flowchart LR
    A[npm run build-core] --> B[wasm-pack build --target web]
    A --> C[cargo build --release]
    B --> D[main/opengeometry/pkg/*]
    E[npm run build-three] --> F[rollup main/opengeometry-three/index.ts]
    D --> F
    F --> G[dist/index.js + d.ts]
    H[npm run copy-wasm] --> I[dist/opengeometry_bg.wasm + package metadata]
    G --> J[dist/ publish artifact]
    I --> J
```

## 5) Practical Notes

- `main/opengeometry` is the actual engine.
- `main/opengeometry-three` is the runtime adapter for Three.js.
- `main/opengeometry-webgl` and `main/opengeometry-babylon` are currently scaffolds (package metadata only).
- Projection + PDF examples in `main/opengeometry/examples/` are the fastest way to inspect real engine behavior.
