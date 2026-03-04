# Boolean Operations System (Kernel + JS Bindings + Three Adapter)

## What changed

This change adds a full boolean operation flow that can be executed from Rust, consumed from wasm/js bindings, and rendered through `@opengeometry/kernel-three`.

### 1) Kernel boolean operation module
- Added `main/opengeometry/src/operations/boolean.rs`.
- Introduced wasm-exported `BooleanOperation` enum:
  - `Union`
  - `Intersection`
  - `Difference`
- Introduced wasm-exported `OGBooleanResult` that:
  - accepts two serialized BReps,
  - computes the result,
  - returns both serialized BRep and triangulated render geometry.

### 2) Boolean algorithm and constraint model
To provide a boolean operation that works across all existing solids with the current project architecture, the implementation uses a **voxel-constrained CSG** pipeline:

1. Convert each input BRep face-loop set to triangles.
2. Build a combined bounding box.
3. Sample voxel centers on a regular grid (`voxel_size` is the constraint parameter).
4. Classify sample points as inside/outside each shape using parity ray-casting against triangle soups.
5. Evaluate boolean logic (`union`, `intersection`, `difference`).
6. Reconstruct a watertight surface by emitting only boundary voxel faces.

This is robust for heterogeneous shape inputs because it does not depend on exact face-face intersection topology repair in floating point. Instead, the user controls precision/performance through `voxel_size`.

### 3) Public API wiring
- Registered the new module in `main/opengeometry/src/lib.rs` via `pub mod boolean;` in operations.

### 4) Three.js adapter update
- Added `main/opengeometry-three/src/operations/boolean.ts` with:
  - `BooleanMesh` class (extends `THREE.Mesh`),
  - `compute(first, second, operation, options)` method,
  - optional constraints (`voxelSize`, `color`, `opacity`),
  - `getBrepData()` pass-through for chaining operations.
- Added operation exports in:
  - `main/opengeometry-three/src/operations/index.ts`
  - `main/opengeometry-three/index.ts`

### 5) Working example
- Added new interactive example:
  - Page: `main/opengeometry-three/examples-vite/src/pages/operations-boolean.ts`
  - HTML entry: `main/opengeometry-three/examples-vite/operations/boolean.html`
- Example lets the user switch operation and voxel constraint interactively.

### 6) Tests
Added kernel tests in `main/opengeometry/src/operations/boolean.rs`:
- union of overlapping cuboids returns non-empty geometry,
- difference of overlapping cuboids returns non-empty geometry.

## Why this changed

The project already has many shape generators that emit BReps but does not yet have a production path for generic shape boolean operations. This patch introduces a single, unified operation path that can accept any shape capable of producing BRep data and gives users a practical constraint knob (`voxel_size`) for balancing robustness and fidelity.

## How to test locally

### Kernel tests
```bash
cargo test --manifest-path main/opengeometry/Cargo.toml
```

### Three adapter lint smoke for the new file
```bash
npx eslint main/opengeometry-three/src/operations/boolean.ts
```

### Example build (optional)
```bash
npm run build-example-three
```
Then open `main/opengeometry-three/examples-dist/operations/boolean.html`.

## Backward compatibility

- Existing shape APIs are unchanged.
- Existing render wrappers are unchanged.
- New functionality is additive.

## Known caveats and follow-ups

1. The current implementation is voxel-based (approximate), not exact analytic BRep-BRep splitting.
2. Output triangle density grows quickly as `voxel_size` decreases.
3. Future follow-up can add adaptive grids, cached spatial acceleration, and exact predicate/splitting stages for high-fidelity CAD workflows.
