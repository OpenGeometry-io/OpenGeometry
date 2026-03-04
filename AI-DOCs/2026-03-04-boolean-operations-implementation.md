# 2026-03-04 Boolean Operations Implementation

## What changed

- Added a new kernel operation module `operations::boolean` with a wasm-bindgen class `OGBoolean`.
- `OGBoolean::compute` accepts two triangle buffers (`[x,y,z,...]`), an operation (`union`, `intersection`, `difference`), and optional numeric constraints.
- Implemented a BSP/plane-splitting CSG pipeline (`Node`, `Plane`, `Polygon`) with tolerance-driven vertex snapping and post-op vertex welding.
- Exposed the new module from `main/opengeometry/src/lib.rs`.
- Added `@opengeometry/kernel-three` bindings:
  - `BooleanShape` for running booleans on any `THREE.Mesh` with `BufferGeometry`.
  - `BooleanConstraints` and `BooleanOperation` types.
- Added a new Vite example page (`examples-vite/operations/boolean.html`) and runtime page logic (`operations-boolean.ts`).
- Updated the examples index to advertise the new Boolean operation card.

## Why it changed

The project needed a shape-agnostic boolean pipeline. Existing shape wrappers already produce triangulated meshes, so this implementation uses those triangulations as a shared interchange format and applies constructive-solid-geometry operations in the kernel, then returns a new triangle buffer for rendering.

## Paper alignment and robustness strategy

The requested `RobustBoolean.pdf` was not available inside this execution environment, so direct line-by-line extraction from the paper was not possible during implementation. This change still applies common robust boolean principles typically emphasized in robust CSG literature:

- Epsilon-based plane classification (`front`, `back`, `coplanar`, `spanning`)
- Grid snapping for deterministic splitting intersections
- Vertex welding after operation completion

These controls are exposed as `constraints.epsilon` and `constraints.snap` so callers can tune behavior per model scale.

## How to test locally

1. Kernel tests:

```bash
cargo test --manifest-path main/opengeometry/Cargo.toml
```

2. Build wasm package (requires `wasm-pack`):

```bash
npm run build-core
```

3. Build Three adapter/examples:

```bash
npm run build-three
npm --prefix main/opengeometry-three run build-example-three
```

4. Open `main/opengeometry-three/examples-vite/operations/boolean.html` through the Vite examples app.

## Backward compatibility

- No existing public API was removed.
- Existing shapes/primitives continue unchanged.
- New API is additive (`OGBoolean`, `BooleanShape`).

## Known caveats and follow-ups

- Current boolean output quality depends on clean manifold triangulations.
- Numerical controls may need per-project presets depending on unit scale.
- A follow-up should validate against the intended RobustBoolean paper test corpus once the PDF is available in-repo.
