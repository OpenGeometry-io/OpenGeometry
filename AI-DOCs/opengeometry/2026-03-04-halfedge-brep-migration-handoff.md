# Half-Edge BREP Migration Handoff

## What Changed

- Replaced the BREP core model with a half-edge-first topology schema.
- Added canonical topology entities:
  - `Vertex { outgoing_halfedge }`
  - `HalfEdge`
  - `Edge { halfedge, twin_halfedge }`
  - `Loop`
  - `Face { outer_loop, inner_loops, shell_ref }`
  - `Wire`
  - `Shell`
- Added `BrepBuilder` for deterministic topology construction.
- Added `Brep::validate_topology() -> Result<(), BrepError>` with manifold and linkage checks.
- Reworked projection/HLR to derive visibility from half-edge adjacency.
- Migrated primitives and sweep/extrude operations to builder-based half-edge output.
- Updated wasm mutating methods to checked APIs (`Result<(), JsValue>`) for invalid topology/input states.
- Updated three integration (`shapes/sphere.ts`) to consume outline buffers from kernel instead of legacy edge endpoint fields.

## Breaking Changes

- `get_brep_serialized()` now emits the new half-edge schema.
- Removed legacy fields from serialized BREP:
  - `edges[].v1`
  - `edges[].v2`
  - `faces[].face_indices`
  - root `holes`
  - root `hole_edges`
- Mutating methods such as `set_config` / `generate_geometry` now return `Result<(), JsValue>` at wasm boundaries.

## Legacy Cleanup

- Removed:
  - `main/opengeometry/src/utility/geometry.rs`
  - `main/opengeometry/src/primitives/cylinderOld.rs`
- Removed stale commented legacy BREP scaffolding from `main/opengeometry/src/lib.rs`.
- Replaced old extrude geometry dependency with local `Geometry` in `operations/extrude.rs`.

## Implementation Notes

- `BrepBuilder` enforces:
  - canonical undirected edge mapping
  - twin linking of opposite directed halfedges
  - non-manifold rejection (`> 2` halfedges per edge)
  - loop closure and link consistency
- Face triangulation inputs now come from `outer_loop` and `inner_loops` via `Brep::get_vertices_and_holes_by_face_id`.
- Wire primitives (`line`, `curve`, `polyline`, `arc`) are represented as `wires` and edge-backed halfedges.
- Solid primitives (`cuboid`, `cylinder`, `wedge`, `sphere`, `sweep`) assign shell topology.

## Files Added

- `main/opengeometry/src/brep/error.rs`
- `main/opengeometry/src/brep/builder.rs`
- `main/opengeometry/src/brep/loop.rs`
- `main/opengeometry/src/brep/wire.rs`
- `main/opengeometry/src/brep/shell.rs`

## Files Reworked (Core)

- `main/opengeometry/src/brep/mod.rs`
- `main/opengeometry/src/brep/vertex.rs`
- `main/opengeometry/src/brep/halfedge.rs`
- `main/opengeometry/src/brep/edge.rs`
- `main/opengeometry/src/brep/face.rs`
- `main/opengeometry/src/export/projection.rs`
- `main/opengeometry/src/operations/extrude.rs`
- `main/opengeometry/src/operations/sweep.rs`

## Files Reworked (Primitives)

- `main/opengeometry/src/primitives/line.rs`
- `main/opengeometry/src/primitives/curve.rs`
- `main/opengeometry/src/primitives/polyline.rs`
- `main/opengeometry/src/primitives/arc.rs`
- `main/opengeometry/src/primitives/rectangle.rs`
- `main/opengeometry/src/primitives/polygon.rs`
- `main/opengeometry/src/primitives/cuboid.rs`
- `main/opengeometry/src/primitives/cylinder.rs`
- `main/opengeometry/src/primitives/wedge.rs`
- `main/opengeometry/src/primitives/sphere.rs`
- `main/opengeometry/src/primitives/sweep.rs`

## Validation Run

- `cargo fmt --manifest-path main/opengeometry/Cargo.toml`
- `cargo check --manifest-path main/opengeometry/Cargo.toml`
- `cargo test --manifest-path main/opengeometry/Cargo.toml`
- `cargo test --examples --manifest-path main/opengeometry/Cargo.toml`
- `npm run build-three`

All commands above completed successfully.

## Known Caveats / Follow-ups

- Existing non-critical warnings still present in unrelated legacy code:
  - `operations/windingsort.rs` (`ccw_and_flag` naming)
  - `geometry/triangle.rs` (`crso` unused variable)
- Consumers that deserialize old BREP JSON must migrate to the new half-edge schema.
