# Parametric and Freeform Editor Handoff

Date: 2026-03-22

## What Changed

- Reframed native OpenGeometry wrappers as `parametric` objects edited through `getConfig` / `setConfig` and `getPlacement` / `setPlacement`.
- Renamed the public direct-edit surface from `EditableBrep` terminology to `freeform` terminology:
  - wasm export is now `OGFreeformGeometry`
  - `opengeometry-three` exports `FreeformGeometry`, `createFreeformGeometry()`, and `FreeformEditResult`
- Promoted freeform into a first-class module:
  - Rust implementation now lives under `main/opengeometry/src/freeform/`
  - TypeScript wrapper now lives under `main/opengeometry-three/src/freeform/`
  - `main/opengeometry/src/editor/` remains as a compatibility re-export rather than the primary home
- Added `getEditCapabilities()` and `toFreeform()` across native wrapper types in `opengeometry-three`.
- Added `BooleanResult.toFreeform()` so boolean outputs can be turned into editable freeform geometry without extra adapter code.
- Added `cutFace(faceId, startEdgeId, startT, endEdgeId, endT)` to the freeform kernel and TS wrapper for single-face cuts that split only the selected face.
- Added `loopCut(edgeId, t)` to the freeform kernel and TS wrapper for closed quad edge rings.
- Updated the Vite editor example so:
  - `Cut One Freeform Side Face` demonstrates a Forma-style single-face cut on one cuboid side
  - `Loop Cut Freeform Side Ring` demonstrates a real kernel-backed loop cut on the converted cuboid
  - the older split-and-move workaround is no longer the main path for adding a visible face cut
  - the orange preview now draws topology edges as well as outline edges so coplanar cuts remain visible
- Split the TypeScript editor surface into smaller files under `main/opengeometry-three/src/operations/editor/`.
- Added a reusable scene example and a Vite example showing parametric edits first and explicit freeform conversion second.

## Why It Changed

`editor-controls` needs a clear two-mode model:

- `parametric` mode for config and placement changes on native objects
- `freeform` mode for direct face/edge/vertex editing after explicit conversion

This keeps object-mode interaction logic in the GUI package while preserving one robust direct-edit engine in the kernel.

## How to Test Locally

Run from repository root:

1. `cargo fmt --check --manifest-path main/opengeometry/Cargo.toml`
2. `cargo check --examples --manifest-path main/opengeometry/Cargo.toml`
3. `cargo test -q --manifest-path main/opengeometry/Cargo.toml`
4. `npm test`
5. `npm run build-three`
6. `npm --prefix main/opengeometry-three run build-example-three`

## Backward-Compatibility Notes

- This is intentionally breaking for the editing API surface.
- Public `EditableBrep*` naming is replaced by `freeform` naming.
- The old convenience helpers `describeEditableObject()` and `enterFreeformMode()` were removed to keep the public API thinner.
- Direct BRep editing behavior is preserved; only the product language and integration contract changed.
- Existing imports through `main/opengeometry/src/editor/` and `main/opengeometry-three/src/operations/editor/` continue to work through compatibility re-exports while the dedicated freeform module becomes the primary home.

## Known Caveats

- Object-mode handle math is intentionally not implemented in the kernel or `opengeometry-three`; that belongs in `editor-controls`.
- `Opening` continues to use the cuboid kernel under the hood, but now reports itself as `entityType: "opening"` to the TS editing contract.
- The Rust module previously named `render.rs` is now `topology_display.rs` because it prepares topology display payloads rather than performing rendering.
- Boolean outputs convert to freeform with identity placement and baked world-space coordinates, because boolean results currently serialize world BReps rather than a local-BRep-plus-placement pair.
- `cutFace` is currently the right primitive for a Forma-like single-face split. `loopCut` remains intentionally scoped to closed quad edge rings.
