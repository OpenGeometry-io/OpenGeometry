# 2026-03-04 Boolean Operations Implementation

## What changed

- Added a stateful kernel boolean operation module `operations::boolean` with wasm-bindgen class `OGBoolean`.
- `OGBoolean::compute` now stores the resulting polygon set and returns triangulated mesh buffer output.
- Added a kernel outline export API: `OGBoolean::get_outline_geometry_serialized`.
- Outline generation is done in kernel code by traversing final CSG polygons and emitting deduplicated polygon-boundary edges.
- Added/updated `@opengeometry/kernel-three` bindings:
  - `BooleanShape` now requests both mesh and outline buffers from kernel.
  - Added boolean outline handling (`LineSegments`) with `outline` toggle behavior aligned with other wrapped shapes.
  - Switched operation selection to symbolic enum-style constants (`BooleanOperation.Union`, `BooleanOperation.Intersection`, `BooleanOperation.Difference`) instead of numeric indices.
- Updated boolean example page:
  - Added dropdown for operation selection.
  - Added left/right shape dropdowns (Cuboid, Sphere, Cylinder, Wedge).
  - Updated demo to run booleans across those shapes and display resulting outline.
- Extended example control runtime to support `select` controls in addition to number/boolean controls.

## Why it changed

Follow-up requirements requested parity with other shape wrappers:

- boolean result must have outline support
- operation selection should be enum-based (not numeric)
- example must include additional operand shape types and a dropdown UX

## Robustness strategy

The boolean implementation continues to use robust CSG controls:

- Epsilon-based plane classification (`front`, `back`, `coplanar`, `spanning`)
- Grid snapping for deterministic split points
- Post-op welding via epsilon snapping

These are exposed via `constraints.epsilon` and `constraints.snap`.

## How to test locally

1. Kernel format + tests:

```bash
cargo fmt --manifest-path main/opengeometry/Cargo.toml
cargo test --manifest-path main/opengeometry/Cargo.toml
```

2. TS lint check:

```bash
npm run lint:check
```

3. Build wasm package (requires `wasm-pack`):

```bash
npm run build-core
```

4. Run examples app:

```bash
npm --prefix main/opengeometry-three run dev-example-three
```

Open:

- `main/opengeometry-three/examples-vite/operations/boolean.html`

## Backward compatibility

- Existing shape and primitive wrappers are unchanged.
- Boolean API remains additive.
- `BooleanShape` API remains compatible while adding outline behavior.

## Known caveats and follow-ups

- In this environment, `wasm-pack` is unavailable, so full wasm rebuild and example execution are limited.
- Existing repository lint errors outside boolean changes still cause `npm run lint:check` to fail.
- If very dense meshes are used, polygon edge outlines can be visually heavy; future work can add feature-edge filtering by angle threshold.
