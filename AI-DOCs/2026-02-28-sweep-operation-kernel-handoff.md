# Sweep Operation Handoff

## What Changed

- Added a new Rust sweep algorithm in `main/opengeometry/src/operations/sweep.rs`.
  - Sweeps a profile section across a path using transported local frames.
  - Supports optional `cap_start` / `cap_end` for open paths.
  - Automatically treats repeated start/end path points as a closed loop path.
- Added unit tests for sweep topology in `operations/sweep.rs`.
- Added new wasm-facing primitive `OGSweep` in `main/opengeometry/src/primitives/sweep.rs`.
  - APIs: `set_config`, `set_config_with_caps`, `set_caps`, `generate_geometry`.
  - Outputs: `get_brep_serialized`, `get_geometry_serialized`, `get_outline_geometry_serialized`.
  - Includes Rust-side helpers to seed path/profile from existing primitives (`OGPolyline`, `OGLine`, `OGRectangle`).
- Wired modules in `main/opengeometry/src/lib.rs`.
- Added Rust example `main/opengeometry/examples/sweep_path_profile.rs`.
  - Demonstrates path from `OGPolyline` + profile from `OGRectangle`.
  - Exports projected PDF for quick visual verification.
- Updated `main/opengeometry/cad.md` with new sweep example command.
- Added Three.js shape wrapper `main/opengeometry-three/src/shapes/sweep.ts`.
  - Creates mesh output from kernel sweep geometry.
  - Supports caps and outline rendering.
- Exported sweep shape in `main/opengeometry-three/src/shapes/index.ts`.
- Added Three.js subrepo example `main/opengeometry-three/src/examples/sweep.ts` and barrel export.
  - Demonstrates constructing sweep from existing `Polyline` and `Rectangle` primitives.
- Added a browser HTML demo in `main/opengeometry-three/examples/sweep.html`.
  - Boots the Three.js scene and OpenGeometry wasm, then renders the sweep output with outline.
- Added a new Rust offset operation in `main/opengeometry/src/operations/offset.rs`.
  - Works for open/closed paths in XZ plane.
  - Supports bevel joins for acute corners using a configurable `acute_threshold_degrees`.
- Offset robustness fixes:
  - Closed offset outputs now append the first point at the end when `is_closed = true` so rendered line strips close reliably.
  - Acute beveling now applies only to outward corners for the chosen offset side, preventing inner-corner triangular spikes.
  - Added regression tests for closed-output closure and inner-corner bevel behavior.
- Reworked and exported `OGCurve` primitive with offset support in `main/opengeometry/src/primitives/curve.rs`.
- Added offset APIs to primitives:
  - `OGPolyline::get_offset_serialized(...)`
  - `OGLine::get_offset_serialized(...)`
  - `OGCurve::get_offset_serialized(...)`
  - `OGRectangle::get_offset_serialized(...)`
- Added Rust examples:
  - `main/opengeometry/examples/offset_primitives.rs`
  - `main/opengeometry/examples/wall_from_polyline_offsets.rs`
- Added Three.js offset helper APIs in wrappers:
  - `Polyline.getOffset(...)`
  - `Line.getOffset(...)`
  - `Curve.getOffset(...)`
  - `Rectangle.getOffset(...)`
- Added Three.js example builders:
  - `main/opengeometry-three/src/examples/offset.ts`
  - `main/opengeometry-three/src/examples/wall-from-offsets.ts`
- Added browser HTML demos:
  - `main/opengeometry-three/examples/offset.html`
  - `main/opengeometry-three/examples/wall-from-offsets.html`
- Exported examples from `main/opengeometry-three/index.ts`.
- Three.js offset example robustness fixes:
  - `main/opengeometry-three/src/examples/offset.ts` now uses `isClosed` to normalize renderable offset point lists without double-closing.
  - `main/opengeometry/src/primitives/polyline.rs` `get_geometry_serialized()` now repeats the first vertex for closed polylines so line rendering includes the closing edge.

## Why It Changed

- Required a path+profile sweep operation in the Rust kernel with architecture-consistent primitive exposure.
- Required corresponding Three.js kernel output support and a usage example in each kernel.

## How To Test Locally

From `main/opengeometry`:

```bash
cargo check
cargo test
cargo run --example sweep_path_profile -- ./target/sweep_path_profile_test.pdf
cargo run --example offset_primitives -- ./target/offset_primitives_test.pdf
cargo run --example wall_from_polyline_offsets -- ./target/wall_from_offsets_test.pdf
```

From repository root:

```bash
npm run build
```

To view the HTML sweep demo (served from repository root):

```bash
python3 -m http.server 8080
```

Then open:

```text
http://localhost:8080/main/opengeometry-three/examples/sweep.html
http://localhost:8080/main/opengeometry-three/examples/offset.html
http://localhost:8080/main/opengeometry-three/examples/wall-from-offsets.html
```

## Backward Compatibility

- Existing primitives and operations were not behaviorally changed.
- Sweep/offset are additive:
  - `operations::sweep`, `primitives::sweep`
  - `operations::offset`
  - additional primitive offset methods on `line`, `polyline`, `curve`, and `rectangle`
- Behavior update for closed offsets:
  - Closed offset result point arrays now end with the first point (explicit closure for render-friendly line strips).

## Known Caveats / Follow-Ups

- Existing repository warnings/errors outside sweep scope remain unchanged (for example, lint issues in `main/opengeometry-three/src/utils/event.ts`).
