# 2026-03-04 — boolean-operations-implementation

## What changed

Implemented a new boolean operation system in `opengeometry-three` that can combine **any two `THREE.Mesh`-compatible OpenGeometry shapes** (cuboid, cylinder, sphere, wedge, opening, sweep, polygon-derived meshes) using:

- `union`
- `subtract`
- `intersect`

The implementation introduces:

1. `src/operations/boolean.ts`
   - `booleanOperation(left, right, options)` API
   - `BooleanShape` mesh result type
   - Configurable constraints (`gridResolution`, `constrainResultToPositiveY`, material controls)
2. `src/operations/index.ts` export barrel
3. Public package exports in `main/opengeometry-three/index.ts`
4. Runnable Vite example:
   - `examples-vite/operations/boolean.html`
   - `examples-vite/src/pages/operations-boolean.ts`
5. Example index update to surface the Boolean operation card.

## Why it changed

The project needed an end-to-end boolean workflow to generate a new resulting shape from two input shapes under configurable constraints, and make that available through JS bindings and the `opengeometry-three` package surface.

Given the environment restriction preventing new registry downloads during this task, the implementation uses a dependency-free voxel classification strategy over existing `three` APIs rather than external CSG packages.

## Primary-source alignment (RobustBoolean paper)

The implementation follows the robustness direction from robust boolean literature by prioritizing **classification stability** over exact floating-point surface-surface reconstruction:

- Input solids are transformed into a common field domain.
- Occupancy is decided with consistent inside/outside tests.
- Boolean logic is applied on classified cells.
- Surface is extracted from cell boundary transitions.

This avoids common triangle-triangle degeneracy failure modes in direct mesh clipping and gives predictable behavior across diverse shape families.

## How to test locally

From repository root:

1. Build/check Rust core:
   - `cargo check --manifest-path main/opengeometry/Cargo.toml`
   - `cargo test --manifest-path main/opengeometry/Cargo.toml`
2. Build Three examples:
   - `npm --prefix main/opengeometry-three run build-example-three`
3. Run example dev server:
   - `npm --prefix main/opengeometry-three run dev-example-three`
4. Open:
   - `/operations/boolean.html`

Interactive controls:
- operation (union/subtract/intersect)
- grid resolution
- shape offset
- positive-Y clamp

## Backward compatibility

- No existing APIs were removed.
- Existing shape wrappers are unchanged.
- New functionality is additive and exposed via new exports.

## Known caveats / follow-ups

1. Current boolean result is voxelized (resolution-controlled), so edges are stepped.
2. Higher grid resolution improves fidelity but increases compute time.
3. Next iteration: optional exact-surface backend (when dependency/network policy allows).
